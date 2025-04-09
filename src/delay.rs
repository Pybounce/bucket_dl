
//taken from tokio docs

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

#[derive(Copy, Clone, Eq, PartialEq, PartialOrd, Ord, Hash, Debug)]
pub struct Delay {
    when: Instant,
}

impl Delay {
    pub fn for_duration(time: Duration) -> Self {
        return Self {
            when: Instant::now() + time
        };
    }
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

impl std::fmt::Display for Delay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#?}", self.when)
    }
}
