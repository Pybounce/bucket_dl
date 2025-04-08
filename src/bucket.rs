
use std::{
    error, fs::{File, OpenOptions}, future::Future, io::{Seek, Write}, path::Path, sync::Arc, task::Poll, time::Duration
};
use crate::delay::Delay;
use reqwest::{self, Client};
use tokio::{spawn, sync::{mpsc::{self, Receiver, Sender}, oneshot, watch}};
use futures_util::{Stream, StreamExt};
use std::pin::Pin;

pub struct BucketProgress {
    pub id: u8,
    pub progress: u64
}

pub struct Bucket {
    id: u8,
    /// Current amount of bytes downloaded.
    bytes_download_watcher: watch::Receiver<u64>,
    /// Total amount of bytes in download.
    size: u64, 
    /// Cancels download
    kill_switch: oneshot::Sender<bool>

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
            kill_switch: kill_switch 
        }
    }
    
    pub fn bytes_downloaded(&self) -> u64 {
        return *self.bytes_download_watcher.borrow();
    }

    pub fn bucket_progress(&self) -> BucketProgress {
        return BucketProgress { id: self.id, progress: self.bytes_downloaded() };
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
