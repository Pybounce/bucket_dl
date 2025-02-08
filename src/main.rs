use std::{
    error, fs::File, io::Write, time::Instant
};
use reqwest::{self};

const DOWNLOAD_URL: &str = "";
const FILE_NAME: &str = "file.csv";


async fn linear_download() -> Result<(), Box<dyn error::Error>> {
    let response = reqwest::get(DOWNLOAD_URL).await?
    .bytes()
    .await?;

    let mut downloaded_file = File::create(FILE_NAME)?;
    downloaded_file.write_all(&response)?;
    Ok(())
}


#[tokio::main]
async fn main() {
    let now = Instant::now();

    println!("downloading...");
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
