
use std::fs::OpenOptions;
use std::io::{Seek, Write};
use std::path::PathBuf;
use std::{
    future::Future, task::Poll, time::Duration
};
use crate::delay::Delay;
use reqwest::Client;
use tokio::spawn;
use tokio::sync::{oneshot, watch};
use futures_util::{Stream, StreamExt};
use std::pin::Pin;
use std::fmt;

/// Contains the id for the given bucket, and progress in terms of how many bytes have currently been downloaded.
#[derive(Copy, Clone, Debug, Default)]
pub struct BucketProgress {
    pub id: u8,
    pub byte_progress: u64,
    pub percent_progress: f32
}

impl fmt::Display for BucketProgress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(id: {}: downloaded {} bytes, {}%)", self.id, self.byte_progress, self.percent_progress)
    }
}

#[derive(Debug)]
pub struct Bucket {
    id: u8,
    url: String,
    file_path: PathBuf,
    client: Client,
    /// Current amount of bytes downloaded.
    bytes_downloaded: u64,
    /// Total amount of bytes in download.
    byte_length: u64,
    /// Offset in bytes from the beginning of the file
    byte_start: u64,
    status: BucketStatus,
    download_handler_opt: Option<BucketDownloadHandler>
}

#[derive(Debug, PartialEq)]
pub enum BucketStatus {
    Idle,
    Downloading,
    Finished,
    Err(String)
}

#[derive(Debug)]
pub struct BucketDownloadHandler {
    /// Current amount of bytes downloaded.
    bytes_download_watcher: watch::Receiver<u64>,
    /// Cancels download
    kill_switch: Option<oneshot::Sender<bool>>,
    status_rx: oneshot::Receiver<Result<(), String>>,
}

impl Bucket {
    pub fn new(id: u8, url: String, file_path: PathBuf, client: Client, byte_length: u64, byte_start: u64) -> Self {
        return Self { 
            id: id,
            url,
            file_path,
            client: client,
            bytes_downloaded: 0,
            byte_length: byte_length, 
            status: BucketStatus::Idle,
            download_handler_opt: None,
            byte_start: byte_start,
        }
    }
    
    pub fn start_download(&mut self) {
        if self.status == BucketStatus::Finished { return; }

        self.status = BucketStatus::Downloading;
        if self.download_handler_opt.is_some() { return; }

        let (w_tx, w_rx) = watch::channel::<u64>(0);
        let (ks_tx, ks_rx) = oneshot::channel::<bool>();
        let (status_tx, status_rx) = oneshot::channel::<Result<(), String>>();

        self.download_handler_opt = BucketDownloadHandler {
            bytes_download_watcher: w_rx,
            kill_switch: ks_tx.into(),
            status_rx,
        }.into();

        spawn(download_range(self.client.clone(), self.bytes_downloaded, (self.byte_start + self.bytes_downloaded) as usize, (self.byte_start + self.byte_length - 1) as usize, self.url.clone(), self.file_path.clone(), w_tx, ks_rx, status_tx));
    }

    pub fn pause(&mut self) {
        if self.status != BucketStatus::Downloading { return; }

        self.status = BucketStatus::Idle;
        self.cancel();
        self.download_handler_opt = None;
    }

    pub fn bytes_downloaded(&mut self) -> u64 {
        if let Some(handler) = &self.download_handler_opt {
            self.bytes_downloaded = *handler.bytes_download_watcher.borrow();
        }
        return self.bytes_downloaded;
    }

    pub fn bucket_progress(&mut self) -> BucketProgress {
        return BucketProgress { id: self.id, byte_progress: self.bytes_downloaded(), percent_progress: self.bytes_downloaded() as f32 / self.size() as f32 };
    }

    pub fn finished(&mut self) -> bool {
        return match self.status {
            BucketStatus::Idle => false,
            BucketStatus::Downloading => {
                match &mut self.download_handler_opt {
                    Some(handler) => {
                        match handler.status_rx.try_recv() {
                            Ok(_) => {
                                self.status = BucketStatus::Finished;
                                true
                            },
                            Err(recv_err) => {
                                match recv_err {
                                    oneshot::error::TryRecvError::Empty => false,
                                    oneshot::error::TryRecvError::Closed => {
                                        self.status = BucketStatus::Err("Bucket status tx closed before sending.".to_owned());
                                        true
                                    },
                                }
                            },
                        }
                    },
                    None => {
                        self.status = BucketStatus::Err("Bucket status was Downloading but download_handler was None.".to_owned());
                        true
                    },
                }
 
            },
            BucketStatus::Finished => true,
            BucketStatus::Err(_) => true,
        };
    }

    pub fn size(&self) -> u64 {
        return self.byte_length;
    }

    pub fn cancel(&mut self) {
        if let Some(handler) = &mut self.download_handler_opt {
            if let Some(switch) = handler.kill_switch.take() {
                let _ = switch.send(true);
            }
        }

    }

}

pub struct BucketProgressStream<'a> {
    buckets: &'a mut [Bucket],
    current_index: usize,
    delay: Delay
}

impl<'a> BucketProgressStream<'a> {
    pub fn new(buckets: &'a mut [Bucket]) -> Self {
        return Self {
            buckets: buckets,
            current_index: 0,
            delay: Delay::for_duration(Duration::from_millis(0))
        };
    }
    pub fn empty() -> Self {
        return Self {
            buckets: &mut [],
            current_index: 0,
            delay: Delay::for_duration(Duration::from_millis(0))
        };
    }
}

impl<'a> Stream for BucketProgressStream<'a> {
    type Item = BucketProgress;

    fn poll_next(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {

        match Pin::new(&mut self.delay).poll(cx) {
            Poll::Ready(_) => {
                self.delay = Delay::for_duration(Duration::from_millis(20));
            },
            Poll::Pending => {
                return Poll::Pending;
            },
        }

        let mut finished = true;
        for b in &mut *self.buckets {
            if !b.finished() { finished = false; break; }
        }
        if finished == true {
            return Poll::Ready(None);
        }
        else {
            let current_index = self.current_index;
            let bucket_update = self.buckets[current_index].bucket_progress();
            self.current_index = (self.current_index + 1) % self.buckets.len();
            return Poll::Ready(bucket_update.into());
        }
    }
}


async fn download_range(
    client: Client,
    previously_downloaded_bytes: u64,
    start_byte: usize, 
    end_byte: usize, 
    url: String, 
    file_path: PathBuf, 
    sender: watch::Sender<u64>, 
    mut kill_switch: oneshot::Receiver<bool>, 
    status_sender: oneshot::Sender<Result<(), String>>
) -> Result<(), ()> {

    let range = format!("bytes={}-{}", start_byte, end_byte);
    if let Ok(response) = client.get(url).header("Range", range).send().await {

        let mut error_opt: Result<(), String> = Ok(());

        match kill_switch.try_recv() {
            Ok(_) => {
                error_opt = Err("Bucket killswitch activated.".to_owned());
            },
            Err(err) => {
                if err == oneshot::error::TryRecvError::Closed {
                    error_opt = Err("Bucket killswitch tx dropped before rx".to_owned().into());
                }
            },
        };

        if let Err(error) = error_opt {
            let _ = status_sender.send(Err(error));
            return Ok(());
        }

        let mut file = match OpenOptions::new().write(true).open(&file_path) {
            Ok(file) => file,
            Err(err) => {
                let _ = status_sender.send(Err(format!("Failed to open file. {}", err)));
                return Ok(());
            }
        };
        if let Err(err) = file.seek(std::io::SeekFrom::Start(start_byte as u64)) {
            let _ = status_sender.send(Err(format!("Failed to seek in file. {}", err)));
            return Ok(());
        }

        let mut stream = response.bytes_stream();
        let mut download_offset = previously_downloaded_bytes;


        while let Some(item) = stream.next().await {

            match kill_switch.try_recv() {
                Ok(_) => {
                    error_opt = Err("Bucket killswitch activated.".to_owned());
                    break;
                },
                Err(err) => {
                    if err == oneshot::error::TryRecvError::Closed {
                        error_opt = Err("Bucket killswitch tx dropped before rx".to_owned().into());
                        break;
                    }
                },
            };

            let bytes = item.unwrap();

            let _ = file.write(&bytes).unwrap();

            download_offset += bytes.len() as u64;
            match sender.send(download_offset) {
                Ok(_) => (),
                Err(e) => {
                    println!("error {:?}", e.0);
                },
            }
        }

        if let Err(err) = file.flush() {
            error_opt = Err(format!("Failed to flush file. {}", err).to_owned().into());
        }

        let _ = status_sender.send(error_opt);
        return Ok(());
    }
    
    return Err(());
}