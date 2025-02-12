
use std::{
    error, fs::{File, OpenOptions}, io::{Seek, Write}, path::Path, sync::Arc
};

use reqwest::{self, Client};
use tokio::{spawn, sync::mpsc::{self, Receiver, Sender}};

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use futures_util::StreamExt;


pub struct ChunkProgress {
    pub id: u8,
    pub progress: u64
}

async fn start_download(url: &String, file_path: &String, sender: Sender<ChunkProgress>) -> Result<Vec<usize>, Box<dyn error::Error>> {



    let client = Arc::new(Client::new());
    let head_response = client.head(url).send().await?;
    let headers = head_response.headers();
    let content_length: usize = headers.get("content-length").unwrap().to_str()?.parse::<usize>()?;
    let chunk_size: usize = get_chunk_size(content_length, headers.contains_key("accept-ranges"));

    let mut tasks: Vec<tokio::task::JoinHandle<Result<(), ()>>> = Vec::with_capacity(content_length / chunk_size);
    let _file = File::create(file_path)?;
    let mut chunk_sizes: Vec<usize> = vec![];
    let mut chunk_id: u8 = 0;
    for start_byte in (0..content_length).step_by(chunk_size) {
        let end_byte = (start_byte + chunk_size).min(content_length);



        tasks.push(spawn(download_range(Arc::clone(&client), start_byte, end_byte - 1, url.clone(), file_path.clone(), sender.clone(), chunk_id)));
        chunk_sizes.push(end_byte - start_byte);
        chunk_id += 1;
    }

    return Ok(chunk_sizes);
}

pub async fn download(url: &String, file_path: &String, sender: Sender<ChunkProgress>) -> Result<Vec<usize>, Box<dyn error::Error>>{

    match start_download(&url, &file_path, sender).await {
        Ok(chunk_sizes) => Ok(chunk_sizes),
        Err(x) => {
            //println!("error: {:?}", x);
            undo_all(&file_path);
            return Err(x);
        },
    }
}
    
fn get_chunk_size(content_length: usize, accepts_ranges: bool) -> usize {
    if accepts_ranges == false { return content_length; }
    return (content_length / 6) + 1;
}

fn undo_all(file_path: &str) {
    println!("Deleting unfinished file...");
    std::fs::remove_file(file_path).unwrap();
    println!("Deleted unfinished file.");
}

async fn download_range(client: Arc<Client>, start_byte: usize, end_byte: usize, url: String, file_path: String, sender: Sender<ChunkProgress>, chunk_id: u8) -> Result<(), ()> {
    let range = format!("bytes={}-{}", start_byte, end_byte);
    
    if let Ok(response) = client.get(url).header("Range", range).send().await {

        let mut file = OpenOptions::new().write(true).open(Path::new(&file_path)).unwrap();
        let _ = file.seek(std::io::SeekFrom::Start(start_byte as u64)).unwrap();

        let mut stream = response.bytes_stream();
        let mut download_offset = 0;

        while let Some(item) = stream.next().await {
            let chunk = item.unwrap();

            let _ = file.write(&chunk).unwrap();

            download_offset += chunk.len() as u64;
            sender.send(ChunkProgress { id: chunk_id, progress: download_offset }).await.unwrap();

        }
        file.flush().unwrap();

        return Ok(());
    }
    
    return Err(());
}