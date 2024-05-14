use core::fmt;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::ready;
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Future for the [`fold`](super::StreamExt::fold) method.
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct Reduce<St: Stream, Fut, F> {
        #[pin]
        stream: St,
        f: F,
        accum: Option<St::Item>,
        #[pin]
        future: Option<Fut>,
    }
}

impl<St, Fut, F> fmt::Debug for Reduce<St, Fut, F>
where
    St: fmt::Debug + Stream,
    St::Item: fmt::Debug,
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Fold")
            .field("stream", &self.stream)
            .field("accum", &self.accum)
            .field("future", &self.future)
            .finish()
    }
}

impl<St, Fut, F> Reduce<St, Fut, F>
where
    St: Stream,
    F: FnMut(St::Item, St::Item) -> Fut,
    Fut: Future<Output = St::Item>,
{
    pub(super) fn new(stream: St, f: F) -> Self {
        Self { stream, f, accum: None, future: None }
    }
}

impl<St, Fut, F> FusedFuture for Reduce<St, Fut, F>
where
    St: Stream,
    F: FnMut(St::Item, St::Item) -> Fut,
    Fut: Future<Output = St::Item>,
{
    fn is_terminated(&self) -> bool {
        self.accum.is_none() && self.future.is_none()
    }
}

impl<St, Fut, F> Future for Reduce<St, Fut, F>
where
    St: Stream,
    F: FnMut(St::Item, St::Item) -> Fut,
    Fut: Future<Output = St::Item>,
{
    type Output = St::Item;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        Poll::Ready(loop {
            if let Some(fut) = this.future.as_mut().as_pin_mut() {
                // we're currently processing a future to produce a new accum value
                *this.accum = Some(ready!(fut.poll(cx)));
                this.future.set(None);
            } else if this.accum.is_some() {
                // we're waiting on a new item from the stream
                let res = ready!(this.stream.as_mut().poll_next(cx));
                let a = this.accum.take().unwrap();
                if let Some(item) = res {
                    this.future.set(Some((this.f)(a, item)));
                } else {
                    break a;
                }
            } else {
                panic!("Reduce polled after completion")
            }
        })
    }
}
