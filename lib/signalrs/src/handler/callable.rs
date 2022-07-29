use std::marker::PhantomData;

use futures::Future;

use crate::{error::SignalRError, invocation::HubInvocation, response::ResponseSink};

use super::Handler;

pub trait Callable {
    type Future: Future<Output = Result<(), SignalRError>> + Send;

    fn call(&self, request: HubInvocation, output: ResponseSink) -> Self::Future;
}

#[derive(Debug)]
pub struct IntoCallable<H, T> {
    handler: H,
    cancellable: bool,
    _marker: PhantomData<T>,
}

impl<H, T> IntoCallable<H, T> {
    pub fn new(handler: H, cancellable: bool) -> Self {
        IntoCallable {
            handler,
            cancellable,
            _marker: Default::default(),
        }
    }
}

impl<H, T> Callable for IntoCallable<H, T>
where
    H: Handler<T> + Clone,
{
    type Future = <H as Handler<T>>::Future;

    fn call(&self, request: HubInvocation, output: ResponseSink) -> Self::Future {
        let handler = self.handler.clone();
        handler.call(request, output, self.cancellable)
    }
}
