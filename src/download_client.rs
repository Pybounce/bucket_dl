
use std::{
    error, fs::{File, OpenOptions}, hash::{BuildHasher, Hasher, RandomState}, io::{Seek, Write}, path::{Path, PathBuf}, sync::Arc
};
use crate::{bucket::{Bucket, BucketProgress, BucketProgressStream}, models::DownloadStatus};
use reqwest::{self, Client, header};
use tokio::{spawn, sync::{oneshot::{self, error::TryRecvError}, watch}};
use futures_util::StreamExt;

/// API to access infomation about a new or ongoing request.
/// # Example
/// ```
/// # use bucket_dl::download_client::DownloadClient;
/// # use bucket_dl::bucket::BucketProgressStream;
/// # use bucket_dl::models::DownloadStatus;
/// # use futures_util::StreamExt;
/// # tokio_test::block_on(async {
/// let mut client = DownloadClient::init(&"".to_owned());
/// match client.begin_download().await {
///     Ok(_) => {
///         let mut stream = client.progress_stream();

///         while let Some(bucket_progress) = stream.next().await {
///             // display logic here
///         }
/// 
///         // always check status to confirm successful download.
///         match client.status() {
///             DownloadStatus::Finished => { /* success! */ }
///             _ => { /* something horrible has happened. */ }
///         }
///     },
///     Err(_) => { /* handle error */ }
/// }
/// # })
/// ```
#[derive(Debug, Default)]
pub struct DownloadClient {
    id: u64,
    buckets: Option<Vec<Bucket>>,
    url: String,
    file_path: PathBuf,
    error_msg: Option<String>,
    cancelled: bool,
    finished: bool,
    paused: bool
}

impl DownloadClient {

    /// Simplest way to create a new client.
    /// # Example
    /// ```
    /// # use bucket_dl::download_client::DownloadClient;
    /// let mut client = DownloadClient::init(&"".to_owned());
    /// ```
    pub fn init(url: &String) -> Self {
        let client_id = RandomState::new().build_hasher().finish();
        return Self {
            id: client_id,
            buckets: None,
            url: url.clone(),
            file_path: PathBuf::default(),
            error_msg: None,
            cancelled: false,
            finished: false,
            paused: false
        };
    }

    /// Begins the download, awaiting this only confirms the download has started.<br/>
    /// This could fail at many points such as making a headers request, following by spawning many threads to request the data, hence the Result return type.<br/>
    /// To check if the download as finished, use [`Self::status`].
    pub async fn begin_download(&mut self) -> Result<(), ()> {
        
        if let Ok(file_path) = self.generate_file_path().await {
            self.file_path = file_path;
        }
        else {
            let err_msg = format!("Failed to generate file path");
            self.error_msg = err_msg.into();
            return Err(());
        }

        if try_create_file(&self.file_path) == false {
            let err_msg = format!("Failed to create file at path {:?}", self.file_path.to_str().to_owned());
            self.error_msg = err_msg.into();
            return Err(());
        }
        match start_download(&self.url, &self.file_path).await {
            Ok(b) => {
                self.buckets = b.into();
            },
            Err(e) => {
                let err_msg = format!("Failed to start download. {}", e);
                self.error_msg = err_msg.into();
                return Err(());
            },
        }
        return match self.status() {
            DownloadStatus::Failed(_) => Err(()),
            _ => Ok(())
        };
    }

    /// Use this if you want progress updates during download.<br/>
    /// The stream will break out once the download is complete OR an error occurs.<br/>
    /// To ensure the download was successful after exhausting the stream, use [`Self::status`]
    /// <br/><br/>
    /// If the download has not started, an empty stream will be returned.
    /// 
    /// # Example
    /// ```
    /// # use bucket_dl::download_client::DownloadClient;
    /// # use bucket_dl::bucket::BucketProgressStream;
    /// # use futures_util::StreamExt;
    /// # tokio_test::block_on(async {
    /// # let mut client = DownloadClient::init(&"".to_owned());
    /// let mut stream = client.progress_stream();
    /// # let mut is_empty = true;
    /// while let Some(bucket_progress) = stream.next().await {
    /// #   is_empty = false;
    ///     println!("Bucket with id {}, has downloaded {} bytes!", bucket_progress.id, bucket_progress.byte_progress);
    /// }
    /// # assert!(is_empty == true);
    /// # })
    /// ```
    pub fn progress_stream(&'_ mut self) -> BucketProgressStream<'_> {
        return match self.buckets.as_mut() {
            Some(buckets) => BucketProgressStream::new(buckets),
            None => BucketProgressStream::empty(),
        };
    }

    /// Returns an iterator of the current progress of the buckets. <br/>
    /// Contrast to [`Self::progress_stream`], this will break as soon as it has sent the progress of each bucket, regardless of if they have finished downloading. <br/>
    /// It is here to serve as a synchonous way to get the current progress. <br/>
    pub fn current_progress(&mut self) -> impl Iterator<Item = BucketProgress> {
        return self.buckets.as_mut().into_iter().flat_map(|buckets| buckets.iter_mut().map(|b| b.bucket_progress()));
    }

    /// Allocates a new vec to store bucket sizes.<br/>
    /// Sizes denote the amount of bytes total for this bucket to download, not remaining.<br/>
    /// Can be used to calculate the percentage completion of the download/bucket.
    pub fn bucket_sizes(&mut self) -> Vec<u64> {
        let mut sizes: Vec<u64> = vec![];
        let buckets = self.buckets.as_mut().unwrap();
        for b in buckets {
            sizes.push(b.size());
        }
        return sizes;
    }

    /// Used to verify whether or not a download was successful.<br/>
    /// Currently, this should be checked after the bucket progress stream is exhausted, since it will break out if an error occurs.
    pub fn status(&mut self) -> DownloadStatus {
        if self.error_msg.is_some() {
            if self.cancelled == false {
                self.cancel();
            }
            return DownloadStatus::Failed(self.error_msg.as_ref().unwrap().clone());
        }
        
        if self.cancelled { return DownloadStatus::Cancelled; }
        if self.paused { return DownloadStatus::Paused; }

        match self.buckets.as_mut() {
            Some(buckets) => {
                for bucket in buckets {
                    if !bucket.finished() { return DownloadStatus::InProgress; }
                }
                if !self.finished { self.finalise(); }
                self.finished = true;
                return DownloadStatus::Finished;
            },
            None => return DownloadStatus::NotStarted,
        };
    }

    pub fn cancel(&mut self) {
        if let Some(buckets) = self.buckets.as_mut() {
            for bucket in buckets {
                bucket.cancel();
            }
            self.delete_unfinished_file();
        }
        self.cancelled = true;
    }

    pub fn pause(&mut self) {
        if let Some(buckets) = self.buckets.as_mut() {
            for bucket in buckets {
                bucket.pause();
            }
            self.paused = true;
        }
    }

    pub fn unpause(&mut self) {
        if let Some(buckets) = self.buckets.as_mut() {
            for bucket in buckets {
                bucket.start_download();
            }
        }
        self.paused = false;
    }

}

impl DownloadClient {
    fn delete_unfinished_file(&self) {
        println!("Deleting unfinished file...");
        let _ = std::fs::remove_file(self.file_path.clone());
        println!("Deleted unfinished file.");
    }

    fn finalise(&self) {
        let prefix_to_remove = format!("{}_", self.id);

        if let Some(file_name_os) = self.file_path.file_name() {
            if let Some(file_name_str) = file_name_os.to_str() {
                if let Some(restored_name) = file_name_str.strip_prefix(&prefix_to_remove) {
                    let new_path = self.file_path.with_file_name(restored_name);
                    if let Err(e) = std::fs::rename(&self.file_path, &new_path) {
                        eprintln!("Failed to rename file: {}", e);
                    } else {
                        println!("File renamed to: {:?}", new_path);
                    }
                }
            }
        }
    }

    async fn generate_file_path(&self) -> Result<PathBuf, Box<dyn error::Error>> {
        let client = Client::new();
        let head_response = client.head(&self.url).send().await?;

        let final_url = head_response.url();
        let mut file_name = final_url
            .path_segments()
            .and_then(|segments| segments.last())
            .filter(|s| !s.is_empty())
            .unwrap_or("download.dat")
            .to_string();
        if !file_name.contains('.') { file_name = format!("{}.dat", file_name); }
        file_name = format!("{}_{}", self.id, file_name);
        let directory = dirs::download_dir().unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        let full_file_path = directory.join(file_name);
        return Ok(full_file_path);
    }
}

async fn start_download(url: &String, file_path: &PathBuf) -> Result<Vec<Bucket>, Box<dyn error::Error>> {

    let mut buckets: Vec<Bucket> = vec![];

    let client = Client::new();
    let head_response = client.head(url).send().await?;
    let headers = head_response.headers();

    let content_length: usize = headers.get(header::CONTENT_LENGTH).unwrap().to_str()?.parse::<usize>()?;
    let standard_bucket_size: usize = get_standard_bucket_size(content_length, headers.contains_key(header::ACCEPT_RANGES));

    let mut bucket_id: u8 = 0;
    for start_byte in (0..content_length).step_by(standard_bucket_size) {
        let bucket = start_bucket_download(bucket_id, start_byte, standard_bucket_size, content_length, url, &file_path, &client);
        buckets.push(bucket);
        bucket_id += 1;
    }

    return Ok(buckets);
}

fn start_bucket_download(id: u8, start_byte: usize, standard_bucket_size: usize, content_length: usize, url: &String, file_path: &PathBuf, client: &Client) -> Bucket {
    let end_byte = (start_byte + standard_bucket_size).min(content_length);
    let byte_length = end_byte - start_byte;
    let mut bucket = Bucket::new(id, url.clone(), file_path.clone(), client.clone(), byte_length as u64, start_byte as u64);
    bucket.start_download();
    return bucket;
}

fn try_create_file(file_path: &PathBuf) -> bool {
    return match File::create(file_path) {
        Ok(_) => true,
        Err(_) => false,
    };
}

fn get_standard_bucket_size(content_length: usize, accepts_ranges: bool) -> usize {
    if accepts_ranges == false { return content_length; }
    return (content_length / 6) + 1;
}