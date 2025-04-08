
use std::{
    error, fs::{File, OpenOptions}, io::{Seek, Write}, path::Path, sync::Arc
};
use crate::{bucket::{Bucket, BucketProgressStream}, models::DownloadStatus};
use reqwest::{self, Client};
use tokio::{spawn, sync::{oneshot, watch}};
use futures_util::StreamExt;

/// API to access infomation about a new or ongoing request.
/// # Example
/// ```
/// # use multithreaded_download_manager::download_client::DownloadClient;
/// # use multithreaded_download_manager::bucket::BucketProgressStream;
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
///             DownloadStatus::Finished => { // success! }
///             _ => { // something horrible has happened. }
///         }
///     },
///     Err(_) => { // handle error }
/// }
/// # }
/// ```
#[derive(Debug, Default)]
pub struct DownloadClient {
    buckets: Option<Vec<Bucket>>,
    url: String,
    file_path: String,
}

impl DownloadClient {

    /// Simplest way to create a new client.
    /// # Example
    /// ```
    /// # use multithreaded_download_manager::download_client::DownloadClient;
    /// let mut client = DownloadClient::init(&"".to_owned(), &"".to_owned());
    /// ```
    pub fn init(url: &String, file_path: &String) -> Self {
        return Self {
            buckets: None,
            url: url.clone(),
            file_path: file_path.clone()
        };
    }

    /// Begins the download, awaiting this only confirms the download has started.</br>
    /// This could fail at many points such as making a headers request, following by spawning many threads to request the data, hence the Result return type.</br>
    /// To check if the download as finished, use [`Self::status`].
    pub async fn begin_download(&mut self) -> Result<(), Box<dyn error::Error>>{
        match start_download(&self.url, &self.file_path).await {
            Ok(b) => {
                self.buckets = b.into();
            },
            Err(e) => {
                println!("AHHHHHHHHH")  ;
                return Err(e); },
        }
        return Ok(());
    }

    /// Use this if you want progress updates during download.</br>
    /// The stream will break out once the download is complete OR an error occurs.</br>
    /// To ensure the download was successful after exausting the stream, use [`Self::status`]
    /// </br></br>
    /// If the download has not started, an empty stream will be returned.
    /// 
    /// # Example
    /// ```
    /// # use multithreaded_download_manager::download_client::DownloadClient;
    /// # use multithreaded_download_manager::bucket::BucketProgressStream;
    /// # use futures_util::StreamExt;
    /// # tokio_test::block_on(async {
    /// # let mut client = DownloadClient::init(&"".to_owned(), &"".to_owned());
    /// let mut stream = client.progress_stream();
    /// # let mut is_empty = true;
    /// while let Some(bucket_progress) = stream.next().await {
    /// #   is_empty = false;
    ///     println!("Bucket with id {}, has downloaded {} bytes!", bucket_progress.id, bucket_progress.progress);
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

    /// Allocates a new vec to store bucket sizes.</br>
    /// Sizes denote the amount of bytes total for this bucket to download, not remaining.</br>
    /// Can be used to calculate the percentage completion of the download/bucket.
    pub fn bucket_sizes(&mut self) -> Vec<u64> {
        let mut sizes: Vec<u64> = vec![];
        let buckets = self.buckets.as_mut().unwrap();
        for b in buckets {
            sizes.push(b.size());
        }
        return sizes;
    }

    /// Used to verify whether or not a download was successful.</br>
    /// Currently, this should be checked after the bucket progress stream is exausted, since it will break out if an error occurs.
    pub fn status(&self) -> DownloadStatus {
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
}


async fn start_download(url: &String, file_path: &String) -> Result<Vec<Bucket>, Box<dyn error::Error>> {

    let mut buckets: Vec<Bucket> = vec![];

    let client = Arc::new(Client::new());
    let head_response = client.head(url).send().await?;
    let headers = head_response.headers();
    let content_length: usize = headers.get("content-length").unwrap().to_str()?.parse::<usize>()?;
    let standard_bucket_size: usize = get_bucket_size(content_length, headers.contains_key("accept-ranges"));

    let _file = File::create(file_path)?;
    let mut bucket_id: u8 = 0;
    for start_byte in (0..content_length).step_by(standard_bucket_size) {
        let end_byte = (start_byte + standard_bucket_size).min(content_length);
        let bucket_size = end_byte - start_byte;
        let (w_tx, w_rx) = watch::channel::<u64>(0);
        let (ks_tx, ks_rx) = oneshot::channel::<bool>();
        

        spawn(download_range(Arc::clone(&client), start_byte, end_byte - 1, url.clone(), file_path.clone(), w_tx, ks_rx));
        buckets.push(Bucket::new(
            bucket_id, 
            bucket_size as u64, 
            w_rx, 
            ks_tx
        ));
        bucket_id += 1;
    }

    return Ok(buckets);
}

fn get_bucket_size(content_length: usize, accepts_ranges: bool) -> usize {
    if accepts_ranges == false { return content_length; }
    return (content_length / 6) + 1;
}

fn undo_all(file_path: &str) {
    println!("Deleting unfinished file...");
    std::fs::remove_file(file_path).unwrap();
    println!("Deleted unfinished file.");
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