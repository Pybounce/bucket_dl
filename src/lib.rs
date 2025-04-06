
pub mod models;


use std::{
    error, fs::{File, OpenOptions}, io::{Seek, Write}, path::Path, sync::Arc
};
use reqwest::{self, Client};
use tokio::{spawn, sync::mpsc::{self, Receiver, Sender}};
use futures_util::StreamExt;

use models::{BucketProgress, Update};



pub struct TheClient {
    bucket_sizes: Option<Vec<usize>>,
    reciever: Option<Receiver<BucketProgress>>,
    url: String,
    file_path: String
}

impl TheClient {
    pub fn init(url: &String, file_path: &String) -> Result<Self, Box<dyn error::Error>> {
        return Ok(Self {
            bucket_sizes: None,
            reciever: None,
            url: url.clone(),
            file_path: file_path.clone()
        });
    }

    pub async fn download(&mut self) -> Result<(), Box<dyn error::Error>>{
        let (tx, rx): (Sender<BucketProgress>, Receiver<BucketProgress>) = mpsc::channel(32);
        self.reciever = Some(rx);
        self.bucket_sizes = start_download(&self.url, &self.file_path, tx).await.unwrap().into();
        return Ok(());
    }

    pub async fn progress(&mut self) -> Update {
        return match &self.reciever {
            Some(_) => {
                let x = self.reciever.as_mut().unwrap().recv().await;
                return match x {
                    Some(x) => Update::Progress(x),
                    None => Update::Finished,
                };
            },
            None => Update::NotStarted,
        };
    }

    pub fn bucket_sizes(&self) -> &Option<Vec<usize>> {
        return &self.bucket_sizes;
    }
    
}


async fn start_download(url: &String, file_path: &String, sender: Sender<BucketProgress>) -> Result<Vec<usize>, Box<dyn error::Error>> {

    let client = Arc::new(Client::new());
    let head_response = client.head(url).send().await?;
    let headers = head_response.headers();
    let content_length: usize = headers.get("content-length").unwrap().to_str()?.parse::<usize>()?;
    let bucket_size: usize = get_bucket_size(content_length, headers.contains_key("accept-ranges"));

    let _file = File::create(file_path)?;
    let mut bucket_sizes: Vec<usize> = vec![];
    let mut bucket_id: u8 = 0;
    for start_byte in (0..content_length).step_by(bucket_size) {
        let end_byte = (start_byte + bucket_size).min(content_length);
        spawn(download_range(Arc::clone(&client), start_byte, end_byte - 1, url.clone(), file_path.clone(), sender.clone(), bucket_id));
        bucket_sizes.push(end_byte - start_byte);
        bucket_id += 1;
    }

    return Ok(bucket_sizes);
}

pub async fn download(url: &String, file_path: &String) -> Result<(Vec<usize>, Receiver<BucketProgress>), Box<dyn error::Error>>{
    let (tx, rx): (Sender<BucketProgress>, Receiver<BucketProgress>) = mpsc::channel(32);

    match start_download(&url, &file_path, tx).await {
        Ok(bucket_sizes) => Ok((bucket_sizes, rx)),
        Err(x) => {
            //println!("error: {:?}", x);
            undo_all(&file_path);
            return Err(x);
        },
    }
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

async fn download_range(client: Arc<Client>, start_byte: usize, end_byte: usize, url: String, file_path: String, sender: Sender<BucketProgress>, bucket_id: u8) -> Result<(), ()> {
    let range = format!("bytes={}-{}", start_byte, end_byte);
    
    if let Ok(response) = client.get(url).header("Range", range).send().await {

        let mut file = OpenOptions::new().write(true).open(Path::new(&file_path)).unwrap();
        let _ = file.seek(std::io::SeekFrom::Start(start_byte as u64)).unwrap();

        let mut stream = response.bytes_stream();
        let mut download_offset = 0;

        while let Some(item) = stream.next().await {
            let bucket = item.unwrap();

            let _ = file.write(&bucket).unwrap();

            download_offset += bucket.len() as u64;
            sender.send(BucketProgress { id: bucket_id, progress: download_offset }).await.unwrap();

        }
        file.flush().unwrap();

        return Ok(());
    }
    
    return Err(());
}