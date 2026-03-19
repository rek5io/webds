use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

pub async fn try_poll_once<T>(fut: impl Future<Output = T>) -> Option<T> {
    struct PollOnce<F> {
        fut: F,
    }

    impl<T, F> Future for PollOnce<F>
    where
        F: Future<Output = T>,
    {
        type Output = Option<T>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            //SAFETY: fut is not moved in memory
            let fut = unsafe { self.map_unchecked_mut(|s| &mut s.fut) };

            match fut.poll(cx) {
                Poll::Ready(t) => Poll::Ready(Some(t)),
                Poll::Pending => Poll::Ready(None),
            }
        }
    }

    (PollOnce { fut }).await
}
