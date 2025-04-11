
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
    _kill_switch: oneshot::Sender<bool>

    // retry_count
    // last_updated_time
}

impl Bucket {
    pub fn new(id: u8, size: u64, bytes_download_watcher: watch::Receiver<u64>, kill_switch: oneshot::Sender<bool>) -> Self {
        return Self { 
            id: id, 
            bytes_download_watcher: 
            bytes_download_watcher, 
            size: size, 
            _kill_switch: kill_switch 
        }
    }
    
    pub fn bytes_downloaded(&self) -> u64 {
        return *self.bytes_download_watcher.borrow();
    }

    pub fn bucket_progress(&self) -> BucketProgress {
        return BucketProgress { id: self.id, byte_progress: self.bytes_downloaded(), percent_progress: self.bytes_downloaded() as f32 / self.size() as f32 };
    }

    pub fn finished(&self) -> bool {
        return self.bytes_downloaded() >= self.size;
    }

    pub fn size(&self) -> u64 {
        return self.size;
    }
}

pub struct BucketProgressStream<'a> {
    buckets: &'a [Bucket],
    current_index: usize,
    delay: Delay
}

impl<'a> BucketProgressStream<'a> {
    pub fn new(buckets: &'a [Bucket]) -> Self {
        return Self {
            buckets: buckets,
            current_index: 0,
            delay: Delay::for_duration(Duration::from_millis(0))
        };
    }
    pub fn empty() -> Self {
        return Self {
            buckets: &[],
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
        for b in self.buckets {
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
