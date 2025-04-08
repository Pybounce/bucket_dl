
//taken from tokio docs

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

pub struct Delay {
    when: Instant,
}

impl Future for Delay {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>)
        -> Poll<()>
    {
        if Instant::now() >= self.when {
            Poll::Ready(())
        } else {
            // Ignore this line for now.
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

impl Delay {
    pub fn for_duration(time: Duration) -> Self {
        return Self {
            when: Instant::now() + time
        };
    }
}