
use std::{
    future::Future, task::Poll, time::Duration
};
use crate::delay::Delay;
use tokio::sync::{oneshot, watch};
use futures_util::Stream;
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
    /// Current amount of bytes downloaded.
    bytes_download_watcher: watch::Receiver<u64>,
    /// Total amount of bytes in download.
    size: u64, 
    /// Cancels download
    kill_switch: Option<oneshot::Sender<bool>>,
    status_rx: oneshot::Receiver<Result<(), String>>,
    status_opt: Option<Result<(), String>>
    // retry_count
    // last_updated_time
}

impl Bucket {
    pub fn new(id: u8, size: u64, bytes_download_watcher: watch::Receiver<u64>, kill_switch: oneshot::Sender<bool>, status_rx: oneshot::Receiver<Result<(), String>>) -> Self {
        return Self { 
            id: id, 
            bytes_download_watcher: 
            bytes_download_watcher, 
            size: size, 
            kill_switch: Some(kill_switch),
            status_rx,
            status_opt: None
        }
    }
    
    pub fn bytes_downloaded(&self) -> u64 {
        return *self.bytes_download_watcher.borrow();
    }

    pub fn bucket_progress(&self) -> BucketProgress {
        return BucketProgress { id: self.id, byte_progress: self.bytes_downloaded(), percent_progress: self.bytes_downloaded() as f32 / self.size() as f32 };
    }

    pub fn finished(&mut self) -> bool {
        if self.status_opt.is_some() { return true; }
        match self.status_rx.try_recv() {
            Ok(_) => {
                self.status_opt = Some(Ok(()));
            },
            Err(recv_err) => {
                match recv_err {
                    oneshot::error::TryRecvError::Empty => return false,
                    oneshot::error::TryRecvError::Closed => {
                        self.status_opt = Some(Err("Bucket status tx closed before sending.".to_owned()));
                        return true;
                    },
                };
            },
        };
        return false;
    }

    pub fn size(&self) -> u64 {
        return self.size;
    }

    pub fn cancel(&mut self) {
        if let Some(switch) = self.kill_switch.take() {
            let _ = switch.send(true);
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
            
            let bucket_update = self.buckets[self.current_index].bucket_progress();
            self.current_index = (self.current_index + 1) % self.buckets.len();
            return Poll::Ready(bucket_update.into());
        }
    }
}
