
use multithreaded_download_manager::download;
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

            let (chunk_sizes, mut rx) = download(&url, &file_path).await.unwrap();
            let mut progress_bars = Vec::<ProgressBar>::with_capacity(chunk_sizes.len());

            for chunk_size in chunk_sizes.iter() {
                let pb = mp.add(ProgressBar::new(*chunk_size as u64));
                pb.set_style(sty.clone());
                pb.set_position(0);
                progress_bars.push(pb);
            }
            while let Some(chunk_progress) = rx.recv().await {
                progress_bars[chunk_progress.id as usize].set_position(chunk_progress.progress);
                
                if chunk_sizes[chunk_progress.id as usize] <= chunk_progress.progress as usize {
                    progress_bars[chunk_progress.id as usize].finish();
                }
            }

            println!("DONE");

        },
        Err(_) => {
            println!("Error parsing input.")
        },
    }
}

