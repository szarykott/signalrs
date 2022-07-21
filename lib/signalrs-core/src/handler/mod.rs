use std::{marker::PhantomData, pin::Pin, sync::Arc};

use futures::Future;

use crate::{
    error::SignalRError,
    extract::FromRequest,
    protocol::{Id, OptionalId},
    request::HubRequest,
    response::{HubResponse, ResponseSink},
};

pub trait Handler<T> {
    type Future: Future<Output = Result<(), SignalRError>> + Send;

    fn call(self, request: HubRequest, output: ResponseSink, stream: bool) -> Self::Future;
}

//     // let result = hub_function(arguments).forward(invocation_id.clone(), output);

//     todo!();

//     // let invocation_id_clone = invocation_id.clone();
//     // let invocations_clone = Arc::clone(&invocations);
//     // let ongoing = tokio::spawn(async move {
//     //     result.await.unwrap();
//     //     let mut invocations = invocations_clone.lock().await;
//     //     (*invocations).remove(&invocation_id_clone);
//     // });

//     // let mut guard = invocations.lock().await;
//     // (*guard).insert(invocation_id, ongoing);

impl<Fn, Fut, Ret, T1> Handler<T1> for Fn
where
    Fn: FnOnce(T1) -> Fut + Send + 'static,
    Fut: Future<Output = Ret> + Send,
    Ret: HubResponse + Send + 'static,
    T1: FromRequest + Send + 'static,
{
    type Future = Pin<Box<dyn Future<Output = Result<(), SignalRError>> + Send>>;

    fn call(self, mut request: HubRequest, output: ResponseSink, stream: bool) -> Self::Future {
        if stream {
            return Box::pin(async move {
                let t1 = FromRequest::try_from_request(&mut request)?;

                let result = (self)(t1).await;

                let id: Id = serde_json::from_str(&request.unwrap_text())?;

                let inflight_map_arc = Arc::clone(&request.hub_state.inflight_invocations);

                let mut inflight_map = request.hub_state.inflight_invocations.lock().await;

                let id_clone = id.invocation_id.clone();
                let inflight = tokio::spawn(async move {
                    let _ = result.forward(id_clone.clone(), output).await; // TODO: How to forward error?
                    let mut inflight_map = inflight_map_arc.lock().await;
                    (*inflight_map).remove(&id_clone);
                });

                (*inflight_map).insert(id.invocation_id, inflight);

                Ok(())
            });
        } else {
            Box::pin(async move {
                let t1 = FromRequest::try_from_request(&mut request)?;

                let result = (self)(t1).await;

                let invocation: OptionalId = serde_json::from_str(&request.unwrap_text())?;
                if let Some(id) = invocation.invocation_id {
                    result.forward(id, output).await?;
                }

                Ok(())
            })
        }
    }
}

pub trait Callable {
    type Future: Future<Output = Result<(), SignalRError>> + Send;

    fn call(&self, request: HubRequest, output: ResponseSink) -> Self::Future;
}

#[derive(Debug)]
pub struct IntoCallable<H, T> {
    handler: H,
    stream: bool,
    _marker: PhantomData<T>,
}

impl<H, T> IntoCallable<H, T> {
    pub fn new(handler: H, stream: bool) -> Self {
        IntoCallable {
            handler,
            stream,
            _marker: Default::default(),
        }
    }
}

impl<H, T> Callable for IntoCallable<H, T>
where
    H: Handler<T> + Clone,
{
    type Future = <H as Handler<T>>::Future;

    fn call(&self, request: HubRequest, output: ResponseSink) -> Self::Future {
        let handler = self.handler.clone();
        handler.call(request, output, self.stream)
    }
}
