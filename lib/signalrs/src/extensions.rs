use futures::stream::Stream;
use pin_project::pin_project;
use std::task::Poll;

pub trait StreamExtR: Stream {
    fn take_while_inclusive<F>(self, f: F) -> TakeWhileInclusive<Self, F>
    where
        F: FnMut(&Self::Item) -> bool,
        Self: Sized,
    {
        TakeWhileInclusive::new(self, f)
    }

    fn chain_if<P, S2>(self, condition: P, next: S2) -> ChainIf<P, Self, S2>
    where
        P: Fn(&Self::Item) -> bool,
        Self: Sized,
    {
        ChainIf::new(self, condition, next)
    }
}

impl<T> StreamExtR for T where T: Stream {}

#[derive(Debug)]
#[pin_project]
pub struct TakeWhileInclusive<S, P> {
    #[pin]
    stream: S,
    predicate: P,
    finish_next_poll: bool,
}

impl<S, P> TakeWhileInclusive<S, P> {
    pub fn new(stream: S, predicate: P) -> Self {
        TakeWhileInclusive {
            stream,
            predicate,
            finish_next_poll: false,
        }
    }
}

impl<S, P> Stream for TakeWhileInclusive<S, P>
where
    S: Stream,
    P: FnMut(&S::Item) -> bool,
{
    type Item = S::Item;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = self.project();

        match this.stream.poll_next(cx) {
            Poll::Ready(Some(item)) => {
                let was_terminated = *this.finish_next_poll;

                if !(this.predicate)(&item) {
                    *this.finish_next_poll = true;
                }

                if was_terminated {
                    Poll::Ready(None)
                } else {
                    Poll::Ready(Some(item))
                }
            }
            x => x,
        }
    }
}

#[derive(Debug)]
#[pin_project]
pub struct ChainIf<P, S1, S2> {
    condition: P,
    #[pin]
    stream: S1,
    #[pin]
    next_stream: S2,
    last_satisfied: bool,
}

impl<P, S1, S2> ChainIf<P, S1, S2> {
    pub fn new(this: S1, condition: P, next: S2) -> Self {
        ChainIf {
            last_satisfied: false,
            stream: this,
            next_stream: next,
            condition,
        }
    }
}

impl<P, S1, S2> Stream for ChainIf<P, S1, S2>
where
    S1: Stream,
    S2: Stream<Item = S1::Item>,
    P: Fn(&S1::Item) -> bool,
{
    type Item = S1::Item;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let this = self.project();

        match this.stream.poll_next(cx) {
            Poll::Ready(Some(i)) => {
                *this.last_satisfied = (this.condition)(&i);
                Poll::Ready(Some(i))
            }
            Poll::Ready(None) => {
                if *this.last_satisfied {
                    this.next_stream.poll_next(cx)
                } else {
                    Poll::Ready(None)
                }
            }
            x => x,
        }
    }
}
