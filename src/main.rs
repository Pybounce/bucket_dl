use std::{
    error, fs::File, io::Write, sync::Arc, time::Instant
};

use bytes::{Bytes, BytesMut};
use reqwest::{self, Client};
use tokio::spawn;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use futures_util::StreamExt;

const DOWNLOAD_URL: &str = "";
const FILE_NAME: &str = "";


async fn linear_download() -> Result<(), Box<dyn error::Error>> {
    let response = reqwest::get(DOWNLOAD_URL).await?
    .bytes()
    .await?;
    
    let mut downloaded_file = File::create(FILE_NAME)?;
    downloaded_file.write_all(&response)?;
    Ok(())
}


async fn download() -> Result<(), Box<dyn error::Error>> {

    let mp = MultiProgress::new();
    let sty = ProgressStyle::with_template(
        "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
    )
    .unwrap()
    .progress_chars("##-");



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

                let pb = mp.add(ProgressBar::new((end_byte - start_byte) as u64));
                pb.set_message("downloading...");
                pb.set_style(sty.clone());
                pb.set_position(0);
                tasks.push(spawn(download_range(Arc::clone(&client), start_byte, end_byte - 1, Arc::new(pb))));
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

async fn download_range(client: Arc<Client>, start_byte: usize, end_byte: usize, pb: Arc<ProgressBar>) -> Result<Bytes, ()> {
    let range = format!("bytes={}-{}", start_byte, end_byte);
    
    if let Ok(response) = client.get(DOWNLOAD_URL).header("Range", range).send().await {

        let mut stream = response.bytes_stream();
        let mut bytes = BytesMut::new();

        while let Some(item) = stream.next().await {
            let chunk = item.unwrap();
            pb.inc(chunk.len() as u64);
            bytes.extend_from_slice(&chunk);
        }
        pb.finish_with_message("done");

        return Ok(bytes.freeze());
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
    
}
