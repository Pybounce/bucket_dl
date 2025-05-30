
use std::{
    error, fs::{File, OpenOptions}, io::{Seek, Write}, path::Path, sync::Arc
};
use crate::{bucket::{Bucket, BucketProgress, BucketProgressStream}, models::DownloadStatus};
use reqwest::{self, Client};
use tokio::{spawn, sync::{oneshot, watch}};
use futures_util::StreamExt;

/// API to access infomation about a new or ongoing request.
/// # Example
/// ```
/// # use bucket_dl::download_client::DownloadClient;
/// # use bucket_dl::bucket::BucketProgressStream;
/// # use bucket_dl::models::DownloadStatus;
/// # use futures_util::StreamExt;
/// # tokio_test::block_on(async {
/// let mut client = DownloadClient::init(&"".to_owned(), &"".to_owned());
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
    buckets: Option<Vec<Bucket>>,
    url: String,
    file_path: String,
    error_msg: Option<String>,
    cancelled: bool
}

impl DownloadClient {

    /// Simplest way to create a new client.
    /// # Example
    /// ```
    /// # use bucket_dl::download_client::DownloadClient;
    /// let mut client = DownloadClient::init(&"".to_owned(), &"".to_owned());
    /// ```
    pub fn init(url: &String, file_path: &String) -> Self {
        return Self {
            buckets: None,
            url: url.clone(),
            file_path: file_path.clone(),
            error_msg: None,
            cancelled: false
        };
    }

    /// Begins the download, awaiting this only confirms the download has started.<br/>
    /// This could fail at many points such as making a headers request, following by spawning many threads to request the data, hence the Result return type.<br/>
    /// To check if the download as finished, use [`Self::status`].
    pub async fn begin_download(&mut self) -> Result<(), ()>{
        if try_create_file(&self.file_path) == false {
            let err_msg = format!("Failed to create file at path {}", self.file_path);
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
    /// # let mut client = DownloadClient::init(&"".to_owned(), &"".to_owned());
    /// let mut stream = client.progress_stream();
    /// # let mut is_empty = true;
    /// while let Some(bucket_progress) = stream.next().await {
    /// #   is_empty = false;
    ///     println!("Bucket with id {}, has downloaded {} bytes!", bucket_progress.id, bucket_progress.byte_progress);
    /// }
    /// # assert!(is_empty == true);
    /// # })
    /// ```
    pub fn progress_stream(&self) -> BucketProgressStream {
        return match self.buckets.as_ref() {
            Some(buckets) => BucketProgressStream::new(buckets),
            None => BucketProgressStream::empty(),
        };
    }

    /// Returns an iterator of the current progress of the buckets. <br/>
    /// Contrast to [`Self::progress_stream`], this will break as soon as it has sent the progress of each bucket, regardless of if they have finished downloading. <br/>
    /// It is here to serve as a synchonous way to get the current progress. <br/>
    pub fn current_progress(&self) -> impl Iterator<Item = BucketProgress> {
        return self.buckets.as_ref().into_iter().flat_map(|buckets| buckets.iter().map(|b| b.bucket_progress()));
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
        if self.cancelled {
            return DownloadStatus::Cancelled;
        }
        match self.buckets.as_ref() {
            Some(buckets) => {
                for bucket in buckets {
                    if !bucket.finished() { return DownloadStatus::InProgress; }
                }
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

}

impl DownloadClient {
    fn delete_unfinished_file(&self) {
        println!("Deleting unfinished file...");
        std::fs::remove_file(self.file_path.clone()).unwrap();
        println!("Deleted unfinished file.");
    }
}

async fn start_download(url: &String, file_path: &String) -> Result<Vec<Bucket>, Box<dyn error::Error>> {

    let mut buckets: Vec<Bucket> = vec![];

    let client = Arc::new(Client::new());
    let head_response = client.head(url).send().await?;
    let headers = head_response.headers();
    let content_length: usize = headers.get("content-length").unwrap().to_str()?.parse::<usize>()?;
    let standard_bucket_size: usize = get_standard_bucket_size(content_length, headers.contains_key("accept-ranges"));

    let mut bucket_id: u8 = 0;
    for start_byte in (0..content_length).step_by(standard_bucket_size) {
        let bucket = start_bucket_download(bucket_id, start_byte, standard_bucket_size, content_length, url, file_path, &client).await;
        buckets.push(bucket);
        bucket_id += 1;
    }

    return Ok(buckets);
}

async fn start_bucket_download(id: u8, start_byte: usize, standard_bucket_size: usize, content_length: usize, url: &String, file_path: &String, client: &Arc<Client>) -> Bucket {
    let end_byte = (start_byte + standard_bucket_size).min(content_length);
    let bucket_size = end_byte - start_byte;
    let (w_tx, w_rx) = watch::channel::<u64>(0);
    let (ks_tx, ks_rx) = oneshot::channel::<bool>();

    spawn(download_range(Arc::clone(client), start_byte, end_byte - 1, url.clone(), file_path.clone(), w_tx, ks_rx));
    return Bucket::new(
        id, 
        bucket_size as u64, 
        w_rx, 
        ks_tx
    );
}

fn try_create_file(file_path: &String) -> bool {
    return match File::create(file_path) {
        Ok(_) => true,
        Err(_) => false,
    };
}

fn get_standard_bucket_size(content_length: usize, accepts_ranges: bool) -> usize {
    if accepts_ranges == false { return content_length; }
    return (content_length / 6) + 1;
}



async fn download_range(client: Arc<Client>, start_byte: usize, end_byte: usize, url: String, file_path: String, sender: watch::Sender<u64>, mut kill_switch: oneshot::Receiver<bool>) -> Result<(), ()> {
    let range = format!("bytes={}-{}", start_byte, end_byte);
    if let Ok(response) = client.get(url).header("Range", range).send().await {

        let mut file = OpenOptions::new().write(true).open(Path::new(&file_path)).unwrap();
        let _ = file.seek(std::io::SeekFrom::Start(start_byte as u64)).unwrap();

        let mut stream = response.bytes_stream();
        let mut download_offset = 0;

        while let Some(item) = stream.next().await {

            if let Ok(kill) = kill_switch.try_recv() {
                if kill { break; }
            }

            let bytes = item.unwrap();

            let _ = file.write(&bytes).unwrap();

            download_offset += bytes.len() as u64;
            match sender.send(download_offset) {
                Ok(_) => (),
                Err(e) => {
                    println!("error {:?}", e.0);
                },
            }
        }

        file.flush().unwrap();

        return Ok(());
    }
    
    return Err(());
}