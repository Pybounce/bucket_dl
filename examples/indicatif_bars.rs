
use futures_util::StreamExt;
use bucket_dl::{download_client::DownloadClient, models::DownloadStatus};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

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

}

#[tokio::main]
async fn main() {
    match parse_input() {
        
        Ok((url, file_path)) => {   
            let mut client = DownloadClient::init(&url, &file_path);

            if let Ok(_) = client.begin_download().await {
                let bucket_sizes = client.bucket_sizes();

                let mp = MultiProgress::new();
                let mut progress_bars = Vec::<ProgressBar>::with_capacity(bucket_sizes.len());

                let sty = ProgressStyle::with_template(
                    "[{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})",
                )
                .unwrap()
                .progress_chars("##-");
    
                for i in 0..bucket_sizes.len() {
                    let bucket_size = bucket_sizes[i];
                    let pb = mp.add(ProgressBar::new(bucket_size as u64));
                    pb.set_style(sty.clone());
                    pb.set_position(0);
                    progress_bars.push(pb);
                }

                let mut bucket_prog_stream = client.progress_stream();

                while let Some(bucket_progress) = bucket_prog_stream.next().await {
                    if progress_bars[bucket_progress.id as usize].is_finished() { continue; }   // should not be indexing based on id since we have no knowledge on hows it's generated

                    progress_bars[bucket_progress.id as usize].set_position(bucket_progress.progress);
    
                    if bucket_sizes[bucket_progress.id as usize] <= bucket_progress.progress {
                        progress_bars[bucket_progress.id as usize].finish();
                    }
                }  
                for pb in progress_bars {
                    pb.finish();
                }                  
            }

            match client.status() {
                DownloadStatus::Finished => {
                    println!("FINISHED")
                },
                _ => println!("Status not finished")
            }
        },
        Err(_) => {
            println!("Error parsing input.")
        },
    }




    
}

