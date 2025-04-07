
pub mod models;


use std::{
    error, fs::{File, OpenOptions}, io::{Seek, Write}, path::Path, sync::Arc
};
use reqwest::{self, Client};
use tokio::{spawn, sync::{mpsc::{self, Receiver, Sender}, oneshot, watch}};
use futures_util::StreamExt;

use models::{BucketProgress, Update};

struct Bucket {
    id: u8,
    /// Current amount of bytes downloaded.
    bytes_download_watcher: watch::Receiver<u64>,
    /// Total amount of bytes in download.
    size: u64, 
    /// Cancels download
    kill_switch: oneshot::Sender<bool>

    // retry_count
    // last_updated_time
}

impl Bucket {
    pub fn new(id: u8, size: u64, bytes_download_watcher: watch::Receiver<u64>, kill_switch: oneshot::Sender<bool>) -> Self {
        return Self { 
            id: id, 
            bytes_download_watcher: 
            bytes_download_watcher, 
            size: size, 
            kill_switch: kill_switch 
        }
    }
    
    pub fn bytes_downloaded(&self) -> u64 {
        return *self.bytes_download_watcher.borrow();
    }

    pub fn bucket_progress(&self) -> BucketProgress {
        return BucketProgress { id: self.id, progress: self.bytes_downloaded() };
    }

    pub fn finished(&self) -> bool {
        return self.bytes_downloaded() >= self.size;
    }
}

pub struct TheClient {
    buckets: Option<Vec<Bucket>>,
    url: String,
    file_path: String,
    current_bucket_index: usize
}

impl TheClient {
    pub fn init(url: &String, file_path: &String) -> Result<Self, Box<dyn error::Error>> {
        return Ok(Self {
            buckets: None,
            url: url.clone(),
            file_path: file_path.clone(),
            current_bucket_index: 0
        });
    }

    pub async fn download(&mut self) -> Result<(), Box<dyn error::Error>>{
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

    pub async fn progress(&mut self) -> Update {
        if self.buckets.is_none() { return Update::NotStarted; }

        let mut finished = true;
        for b in self.buckets.as_mut().unwrap() {
            if !b.finished() { finished = false; }
        }
        if finished == true {
            return Update::Finished;
        }
        else {
            tokio::task::yield_now().await;
            let bucket_update = self.buckets.as_mut().unwrap()[self.current_bucket_index].bucket_progress();
            self.current_bucket_index = (self.current_bucket_index + 1) % self.buckets.as_mut().unwrap().len();
            return Update::Progress(bucket_update);
        }
    }

    pub fn bucket_sizes(&mut self) -> Vec<u64> {
        let mut sizes: Vec<u64> = vec![];
        let buckets = self.buckets.as_mut().unwrap();
        for b in buckets {
            sizes.push(b.size);
        }
        return sizes;
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
    if start_byte == 0 {
        println!("end byte {}", end_byte);
    }
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