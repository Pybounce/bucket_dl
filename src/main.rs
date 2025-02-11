use std::{
    error, fs::{File, OpenOptions}, io::{Seek, Write}, path::Path, sync::Arc, time::Instant
};

use reqwest::{self, Client};
use tokio::spawn;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use futures_util::StreamExt;



async fn download(url: &String, file_path: &String) -> Result<(), Box<dyn error::Error>> {

    let mp = MultiProgress::new();
    let sty = ProgressStyle::with_template(
        "[{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})",
    )
    .unwrap()
    .progress_chars("##-");

    let client = Arc::new(Client::new());
    let head_response = client.head(url).send().await?;
    let headers = head_response.headers();
    let content_length: usize = headers.get("content-length").unwrap().to_str()?.parse::<usize>()?;
    let chunk_size: usize = get_chunk_size(content_length, headers.contains_key("accept-ranges"));

    let mut tasks: Vec<tokio::task::JoinHandle<Result<(), ()>>> = Vec::with_capacity(content_length / chunk_size);
    let _file = File::create(file_path)?;

    for start_byte in (0..content_length).step_by(chunk_size) {
        let end_byte = (start_byte + chunk_size).min(content_length);

        let pb = mp.add(ProgressBar::new((end_byte - start_byte) as u64));
        pb.set_style(sty.clone());
        pb.set_position(0);

        tasks.push(spawn(download_range(Arc::clone(&client), start_byte, end_byte - 1, Arc::new(pb), url.clone(), file_path.clone())));
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

fn undo_all(file_path: &str) {
    println!("Deleting unfinished file...");
    std::fs::remove_file(file_path).unwrap();
    println!("Deleted unfinished file.");
}

async fn download_range<'a>(client: Arc<Client>, start_byte: usize, end_byte: usize, pb: Arc<ProgressBar>, url: String, file_path: String) -> Result<(), ()> {
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
            pb.set_position(download_offset);

        }
        file.flush().unwrap();
        pb.finish();

        return Ok(());
    }
    
    return Err(());
}

fn parse_input() -> Result<(String, String), ()> {
    let args: Vec<String> = std::env::args().collect();

    let mut url = "";
    let mut out = "";
    for i in 0..(args.len() - 1) {
        if args[i] == "-url" {
            url = &args[i + 1];
        }
        if args[i] == "-out" {
            out = &args[i + 1];
        }
    }
    if url == "" || out =="" { return Err(()); }
    return Ok((url.to_owned(), out.to_owned()));

    //return Ok(("http://speedtest.tele2.net/1GB.zip".to_owned(), "file.zip".to_owned()));
}

#[tokio::main]
async fn main() {
    match parse_input() {
        Ok((url, file_path)) => {
            let now = Instant::now();
            
            match download(&url, &file_path).await {
                Ok(_) => {
                    let duration = now.elapsed();
                    println!("\nDone in {duration:?}")
                },
                Err(x) => {
                    println!("error: {:?}", x);
                    undo_all(&file_path);
                },
            }
        },
        Err(_) => {
            println!("Error parsing input.")
        },
    }
}
