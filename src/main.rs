use std::{
    error, fs::{File, OpenOptions}, io::{Seek, Write}, sync::Arc, time::Instant
};

use reqwest::{self, Client};
use tokio::spawn;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use futures_util::StreamExt;

const DOWNLOAD_URL: &str = "http://speedtest.tele2.net/1GB.zip";
const FILE_NAME: &str = "file.zip";

async fn download() -> Result<(), Box<dyn error::Error>> {

    let mp = MultiProgress::new();
    let sty = ProgressStyle::with_template(
        "[{elapsed_precise}] {bar:40.cyan/blue} {bytes}/{total_bytes} ({eta})",
    )
    .unwrap()
    .progress_chars("##-");



    let client = Arc::new(Client::new());
    let head_response = client.head(DOWNLOAD_URL).send().await?;
    let headers = head_response.headers();
    let content_length: usize = headers.get("content-length").unwrap().to_str()?.parse::<usize>()?;
    let chunk_size: usize = get_chunk_size(content_length, headers.contains_key("accept-ranges"));

    let mut tasks: Vec<tokio::task::JoinHandle<Result<(), ()>>> = Vec::with_capacity(content_length / chunk_size);
    let _file = File::create(FILE_NAME)?;

    for start_byte in (0..content_length).step_by(chunk_size) {
        let end_byte = (start_byte + chunk_size).min(content_length);

        let pb = mp.add(ProgressBar::new((end_byte - start_byte) as u64));
        pb.set_style(sty.clone());
        pb.set_position(0);
        
        tasks.push(spawn(download_range(Arc::clone(&client), start_byte, end_byte - 1, Arc::new(pb))));
    }

    for task in tasks.iter_mut() {
        let _ = task.await?.unwrap();
    }


    Ok(())
}

fn get_chunk_size(content_length: usize, accepts_ranges: bool) -> usize {
    if accepts_ranges == false { return content_length; }
    return (content_length / 6) + 1;
}

fn undo_all() {
    println!("Deleting unfinished file...");
    std::fs::remove_file(FILE_NAME).unwrap();
    println!("Deleted unfinished file.");
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

            download_offset += chunk.len() as u64;
            pb.set_position(download_offset);

        }
        file.flush().unwrap();
        pb.finish();

        return Ok(());
    }
    
    return Err(());
}

fn compare() -> Result<(), Box<dyn std::error::Error + 'static>> {
    let threaded = std::fs::read_to_string(FILE_NAME).unwrap();
    let norm = std::fs::read_to_string("1GB.zip").unwrap();
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
            undo_all();
        },
    }
    
}
