use std::{
    error, fs::File, io::Write, sync::Arc, task, thread::JoinHandle, time::Instant
};

use bytes::Bytes;
use reqwest::{self, Client};
use tokio::spawn;

const DOWNLOAD_URL: &str = "https://drive.usercontent.google.com/download?id=1qTh7590c_Dd2jh8aRbqu3btyfJVoc2js&export=download&authuser=0&confirm=t&uuid=1140781a-99d2-4840-bfc0-76691c9328bf&at=AIrpjvNSKDFk13NYZHmTSfHrLEF9%3A1739053106402";
const FILE_NAME: &str = "fileB.zip";


async fn linear_download() -> Result<(), Box<dyn error::Error>> {
    let response = reqwest::get(DOWNLOAD_URL).await?
    .bytes()
    .await?;
    
    let mut downloaded_file = File::create(FILE_NAME)?;
    downloaded_file.write_all(&response)?;
    Ok(())
}


async fn download() -> Result<(), Box<dyn error::Error>> {

    let client = Arc::new(Client::new());
    let head_response = client.head(DOWNLOAD_URL).send().await?;
    let headers = head_response.headers();

    if headers.contains_key("accept-ranges") {
        if let Some(header) = headers.get("content-length") {
            let content_length: usize = header.to_str()?.parse::<usize>()?;
            let chunk_size: usize = (content_length / 6) + 1;
            let mut tasks: Vec<tokio::task::JoinHandle<Result<Bytes, ()>>> = Vec::with_capacity(content_length / chunk_size);

            for start_byte in (0..content_length).step_by(chunk_size) {
                let end_byte = (start_byte + chunk_size).min(content_length);
                tasks.push(spawn(download_range(Arc::clone(&client), start_byte, end_byte - 1)));
            }

            let mut bytes: Vec<u8> = Vec::with_capacity(content_length);

            for task in tasks.iter_mut() {
                let a = task.await?.unwrap();
                bytes.extend(a);
            }
            let mut downloaded_file = File::create(FILE_NAME)?;
            downloaded_file.write_all(&bytes)?;
        }
        println!("written with chunks");
    }
    else { 
        println!("does not accept range"); 
    }
    Ok(())
}

async fn download_range(client: Arc<Client>, start_byte: usize, end_byte: usize) -> Result<Bytes, ()> {
    let range = format!("bytes={}-{}", start_byte, end_byte);
    if let Ok(response) = client.get(DOWNLOAD_URL).header("Range", range).send().await {
        if let Ok(bytes) = response.bytes().await {
            return Ok(bytes);
        }
    }
    
    return Err(());

}

#[tokio::main]
async fn main() {

    let now = Instant::now();
    
    println!("downloading multithreaded");

    match download().await {
    Ok(_) => {
        let duration = now.elapsed();
        println!("done in {duration:?}")
    },
    Err(x) => {
        println!("error: {:?}", x);
    },
    }
    
    
    return;


    let now = Instant::now();

    println!("downloading singlethreaded");
    match linear_download().await {
        Ok(_) => {
            let duration = now.elapsed();
            println!("done in {duration:?}")
        },
        Err(x) => {
            println!("error: {:?}", x);
        },
    }

}
