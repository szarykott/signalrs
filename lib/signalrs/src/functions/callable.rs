use std::marker::PhantomData;

use futures::Future;

use crate::{error::SignalRError, invocation::HubInvocation, response::ResponseSink};

use super::{Handler, StreamingHandler};

pub trait Callable {
    type Future: Future<Output = Result<(), SignalRError>> + Send;

    fn call(&self, request: HubInvocation, output: ResponseSink) -> Self::Future;
}

#[derive(Debug)]
pub struct NonStreamingCallable<H, T> {
    handler: H,
    _marker: PhantomData<T>,
}

impl<H, T> NonStreamingCallable<H, T> {
    pub fn new(handler: H) -> Self {
        NonStreamingCallable {
            handler,
            _marker: Default::default(),
        }
    }
}

#[derive(Debug)]
pub struct StreamingCallable<H, T> {
    handler: H,
    _marker: PhantomData<T>,
}

impl<H, T> StreamingCallable<H, T> {
    pub fn new(handler: H) -> Self {
        StreamingCallable {
            handler,
            _marker: Default::default(),
        }
    }
}

impl<H, T> Callable for NonStreamingCallable<H, T>
where
    H: Handler<T> + Clone,
{
    type Future = <H as Handler<T>>::Future;

    fn call(&self, request: HubInvocation, output: ResponseSink) -> Self::Future {
        let handler = self.handler.clone();
        handler.call(request, output)
    }
}

impl<H, T> Callable for StreamingCallable<H, T>
where
    H: StreamingHandler<T> + Clone,
{
    type Future = <H as StreamingHandler<T>>::Future;

    fn call(&self, request: HubInvocation, output: ResponseSink) -> Self::Future {
        let handler = self.handler.clone();
        handler.call_streaming(request, output)
    }
}
