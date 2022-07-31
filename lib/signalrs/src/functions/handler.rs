use crate::{
    error::{CallerError, SignalRError},
    extract::FromInvocation,
    invocation::HubInvocation,
    protocol::Completion,
    response::*,
};
use futures::{pin_mut, select, Future, FutureExt, SinkExt, StreamExt};
use log::*;
use serde::Serialize;
use std::pin::Pin;

pub trait Handler<T> {
    type Future: Future<Output = Result<(), SignalRError>> + Send;

    fn call(self, request: HubInvocation) -> Self::Future;
}

pub trait StreamingHandler<T> {
    type Future: Future<Output = Result<(), SignalRError>> + Send;

    fn call_streaming(self, request: HubInvocation) -> Self::Future;
}

impl<Fn, Fut, Ret> Handler<()> for Fn
where
    Fn: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = Ret> + Send,
    Ret: IntoResponse + Send + 'static,
{
    type Future = Pin<Box<dyn Future<Output = Result<(), SignalRError>> + Send>>;

    fn call(self, request: HubInvocation) -> Self::Future {
        Box::pin(async move {
            tokio::spawn(async move {
                let response = (self)().await;
                if let Err(e) = forward_single(response, request).await {
                    error!("error while calling method {}", e)
                }
            });

            Ok(())
        })
    }
}

impl<Fn, Fut, Ret, T> Handler<T> for Fn
where
    Fn: FnOnce(T) -> Fut + Send + 'static,
    Fut: Future<Output = Ret> + Send,
    Ret: IntoResponse + Send + 'static,
    T: FromInvocation + Send + 'static,
{
    type Future = Pin<Box<dyn Future<Output = Result<(), SignalRError>> + Send>>;

    fn call(self, mut request: HubInvocation) -> Self::Future {
        Box::pin(async move {
            let t = FromInvocation::try_from_invocation(&mut request)?;

            tokio::spawn(async move {
                let response = (self)(t).await;
                if let Err(e) = forward_single(response, request).await {
                    error!("error while calling method {}", e)
                }
            });

            Ok(())
        })
    }
}

impl<Fn, Fut, Ret> StreamingHandler<()> for Fn
where
    Fn: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = Ret> + Send,
    Ret: IntoHubStream + Send + 'static,
{
    type Future = Pin<Box<dyn Future<Output = Result<(), SignalRError>> + Send>>;

    fn call_streaming(self, request: HubInvocation) -> Self::Future {
        Box::pin(async move {
            tokio::spawn(async move {
                let response = (self)().await;
                if let Err(e) = forward_stream(response, request).await {
                    error!("error while calling streaming method {}", e)
                }
            });

            Ok(())
        })
    }
}

impl<Fn, Fut, Ret, T> StreamingHandler<T> for Fn
where
    Fn: FnOnce(T) -> Fut + Send + 'static,
    Fut: Future<Output = Ret> + Send,
    Ret: IntoHubStream + Send + 'static,
    T: FromInvocation + Send + 'static,
{
    type Future = Pin<Box<dyn Future<Output = Result<(), SignalRError>> + Send>>;

    fn call_streaming(self, mut request: HubInvocation) -> Self::Future {
        Box::pin(async move {
            let t = FromInvocation::try_from_invocation(&mut request)?;

            tokio::spawn(async move {
                let response = (self)(t).await;
                if let Err(e) = forward_stream(response, request).await {
                    error!("error while calling streaming method {}", e)
                }
            });

            Ok(())
        })
    }
}

macro_rules! impl_handlersv2 {
    ($($ty:ident),+) => {
        #[allow(non_snake_case)]
        impl<Fn, Fut, Ret, $($ty,)+> Handler<($($ty,)+)> for Fn
        where
            Fn: FnOnce($($ty,)+) -> Fut + Send + 'static,
            Fut: Future<Output = Ret> + Send,
            Ret: IntoResponse + Send + 'static,
            $(
                $ty: FromInvocation + Send + 'static,
            )+
        {
            type Future = Pin<Box<dyn Future<Output = Result<(), SignalRError>> + Send>>;

            fn call(self, mut request: HubInvocation) -> Self::Future {
                Box::pin(async move {
                    $(
                        let $ty = FromInvocation::try_from_invocation(&mut request)?;
                    )+

                    tokio::spawn(async move {
                        let response = (self)($($ty,)+).await;
                        if let Err(e) = forward_single(response, request).await {
                            error!("error while calling method {}", e)
                        }
                    });

                    Ok(())
                })
            }
        }

        #[allow(non_snake_case)]
        impl<Fn, Fut, Ret, $($ty,)+> StreamingHandler<($($ty,)+)> for Fn
        where
            Fn: FnOnce($($ty,)+) -> Fut + Send + 'static,
            Fut: Future<Output = Ret> + Send,
            Ret: IntoHubStream + Send + 'static,
            $(
                $ty: FromInvocation + Send + 'static,
            )+
        {
            type Future = Pin<Box<dyn Future<Output = Result<(), SignalRError>> + Send>>;

            fn call_streaming(self, mut request: HubInvocation) -> Self::Future {
                Box::pin(async move {
                    $(
                        let $ty = FromInvocation::try_from_invocation(&mut request)?;
                    )+

                    tokio::spawn(async move {
                        let response = (self)($($ty,)+).await;
                        if let Err(e) = forward_stream(response, request).await {
                            error!("error while calling streaming method {}", e)
                        }
                    });

                    Ok(())
                })
            }
        }
    };
}

impl_handlersv2!(T1, T2);
impl_handlersv2!(T1, T2, T3);
impl_handlersv2!(T1, T2, T3, T4);
impl_handlersv2!(T1, T2, T3, T4, T5);
impl_handlersv2!(T1, T2, T3, T4, T5, T6);
impl_handlersv2!(T1, T2, T3, T4, T5, T6, T7);
impl_handlersv2!(T1, T2, T3, T4, T5, T6, T7, T8);
impl_handlersv2!(T1, T2, T3, T4, T5, T6, T7, T8, T9);
impl_handlersv2!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
impl_handlersv2!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
impl_handlersv2!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);
impl_handlersv2!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13);

async fn forward_single<Res>(msg: Res, mut invocation: HubInvocation) -> Result<(), SignalRError>
where
    Res: IntoResponse,
    <Res as IntoResponse>::Out: Serialize,
{
    if let Some(invocation_id) = &invocation.invocation_state.invocation_id {
        let completion = msg.into_completion(invocation_id.clone());
        let json = serde_json::to_string(&completion)?;
        invocation.output.send(json).await?;
    }

    Ok(())
}

async fn forward_stream<Res>(stream: Res, mut invocation: HubInvocation) -> Result<(), SignalRError>
where
    Res: IntoHubStream,
{
    let invocation_id = invocation
        .invocation_state
        .invocation_id
        .clone()
        .ok_or_else(|| CallerError::MissingInvocationId)?;

    let stream = stream.into_stream();
    pin_mut!(stream);

    let cancellation_token = invocation.get_cancellation_token().unwrap_or_default();

    loop {
        select! {
            item = stream.next().fuse() => {
                if let Some(item) = item {
                    if item.is_error() {
                        let completion = item.into_completion(invocation_id.clone());
                        let json = serde_json::to_string(&completion)?;
                        invocation.output.send(json).await?;

                        trace!("stream ending with error");

                        return Ok(()); // expected error
                    }

                    let stream_item = item.into_stream_item(invocation_id.clone());
                    let json = serde_json::to_string(&stream_item)?;
                    invocation.output.send(json).await?;

                    trace!("next stream item sent");
                } else {
                    break;
                }
            }
            _ = cancellation_token.cancelled().fuse() => {
                trace!("cancellation triggered");
                break;
            }
        }
    }

    let completion = Completion::<()>::ok(invocation_id);
    let json = serde_json::to_string(&completion)?;
    invocation.output.send(json).await?;

    trace!("stream invocation forwarded successfully");

    Ok(())
}
