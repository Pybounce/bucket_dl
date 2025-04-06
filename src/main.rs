
use multithreaded_download_manager::{download, models::Update, TheClient};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

fn parse_input() -> Result<(String, String), ()> {

    return Ok(("http://speedtest.tele2.net/1GB.zip".to_owned(), "file.zip".to_owned()));


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
            let mp = MultiProgress::new();
            let sty = ProgressStyle::with_template(
                "[{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})",
            )
            .unwrap()
            .progress_chars("##-");

            if let Ok(mut client) = TheClient::init(&url, &file_path) {
                if let Ok(_) = client.download().await {
                    let bucket_sizes = client.bucket_sizes().clone().unwrap();

                    let mut progress_bars = Vec::<ProgressBar>::with_capacity(bucket_sizes.len());

                    for bucket_size in bucket_sizes.iter() {
                        let pb = mp.add(ProgressBar::new(*bucket_size as u64));
                        pb.set_style(sty.clone());
                        pb.set_position(0);
                        progress_bars.push(pb);
                    }

                    loop {
                        match client.progress().await {
                            Update::Progress(bucket_progress) => {
                                progress_bars[bucket_progress.id as usize].set_position(bucket_progress.progress);
                
                                if bucket_sizes[bucket_progress.id as usize] <= bucket_progress.progress as usize {
                                    progress_bars[bucket_progress.id as usize].finish();
                                }
                            },
                            Update::Finished => break,
                            Update::Failed => break,
                            Update::NotStarted => break,
                        }
                    }

                }

            }
            println!("DONE");

        },
        Err(_) => {
            println!("Error parsing input.")
        },
    }
}

