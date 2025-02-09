use std::{
    error, fs::{File, OpenOptions}, io::{Read, Seek, Write}, os::unix::fs::FileExt, sync::Arc, thread, time::{self, Instant}
};

use reqwest::{self, Client};
use tokio::spawn;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use futures_util::StreamExt;

const DOWNLOAD_URL: &str = "http://speedtest.tele2.net/1GB.zip";
const FILE_NAME: &str = "file.zip";


async fn linear_download() -> Result<(), Box<dyn error::Error>> {
    let mut file = File::create(FILE_NAME)?;
    let response = reqwest::get(DOWNLOAD_URL).await?;
    let mut stream = response.bytes_stream();
    let mut offset = 0;
    while let Some(item) = stream.next().await {
        let chunk = item.unwrap();
        let _ = file.write_at(&chunk, offset);
        offset += chunk.len() as u64;
    }
    file.flush().unwrap();
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
            let mut tasks: Vec<tokio::task::JoinHandle<Result<(), ()>>> = Vec::with_capacity(content_length / chunk_size);
            let file = File::create(FILE_NAME)?;

            for start_byte in (0..content_length).step_by(chunk_size) {
                let end_byte = (start_byte + chunk_size).min(content_length);

                let pb = mp.add(ProgressBar::new((end_byte - start_byte) as u64));
                pb.set_message("downloading...");
                pb.set_style(sty.clone());
                pb.set_position(0);
                tasks.push(spawn(download_range(Arc::clone(&client), start_byte, end_byte - 1, Arc::new(pb))));
            }

            for task in tasks.iter_mut() {
                let _ = task.await?.unwrap();
                //handle err returns here
            }
        }
    }
    else { 

        println!("does not accept range"); 
    }
    
    Ok(())
}

async fn download_range(client: Arc<Client>, start_byte: usize, end_byte: usize, pb: Arc<ProgressBar>) -> Result<(), ()> {
    let range = format!("bytes={}-{}", start_byte, end_byte);
    
    if let Ok(response) = client.get(DOWNLOAD_URL).header("Range", range).send().await {

        let mut file = OpenOptions::new().write(true).open(FILE_NAME).unwrap();
        let _ = file.seek(std::io::SeekFrom::Start(start_byte as u64)).unwrap();

        let mut stream = response.bytes_stream();
        let mut download_offset = 0;

        while let Some(item) = stream.next().await {
            let chunk = item.unwrap();

            let _ = file.write(&chunk).unwrap();

            //let _ = file.write_at(&chunk, start_byte as u64 + download_offset);
            download_offset += chunk.len() as u64;
            pb.set_position(download_offset);

        }
        file.flush().unwrap();
        pb.finish_with_message("done");

        return Ok(());
    }
    
    return Err(());
}

use std::fs;
fn compare() -> Result<(), Box<dyn std::error::Error + 'static>> {
    let threaded = fs::read_to_string(FILE_NAME).unwrap();
    let norm = fs::read_to_string("1GB.zip").unwrap();
    assert_eq!(threaded, norm);
    Ok(())
}

#[tokio::main]
async fn main() {
    //match compare() {
    //    Ok(_) => println!("asdad"),
    //    Err(x) => println!("error: {:?}", x),
    //}
    //return;
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
