use futures::{select, Future, FutureExt};
use std::pin::Pin;
use tokio_util::sync::CancellationToken;

use crate::{error::SignalRError, extract::FromInvocation, invocation::HubInvocation, response::*};

use log::*;

pub trait Handler<T> {
    type Future: Future<Output = Result<(), SignalRError>> + Send;

    fn call(self, request: HubInvocation, output: ResponseSink) -> Self::Future;
}

pub trait StreamingHandler<T> {
    type Future: Future<Output = Result<(), SignalRError>> + Send;

    fn call_streaming(self, request: HubInvocation, output: ResponseSink) -> Self::Future;
}

impl<Fn, Fut, Ret> Handler<()> for Fn
where
    Fn: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = Ret> + Send,
    Ret: IntoResponse + Send + 'static,
{
    type Future = Pin<Box<dyn Future<Output = Result<(), SignalRError>> + Send>>;

    fn call(self, request: HubInvocation, output: ResponseSink) -> Self::Future {
        Box::pin(async move {
            tokio::spawn(async move {
                let response = (self)().await;
                forward_single(response, request, output).await
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

    fn call(self, mut request: HubInvocation, output: ResponseSink) -> Self::Future {
        Box::pin(async move {
            let t = FromInvocation::try_from_request(&mut request)?;

            tokio::spawn(async move {
                let response = (self)(t).await;
                forward_single(response, request, output).await
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

    fn call_streaming(self, request: HubInvocation, output: ResponseSink) -> Self::Future {
        Box::pin(async move {
            let ct = request.get_cancellation_token();
            tokio::spawn(async move {
                let response = (self)().await;
                cancellable(ct, forward_stream(response, request, output)).await;
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

    fn call_streaming(self, mut request: HubInvocation, output: ResponseSink) -> Self::Future {
        Box::pin(async move {
            let t = FromInvocation::try_from_request(&mut request)?;

            let ct = request.get_cancellation_token();
            tokio::spawn(async move {
                let response = (self)(t).await;
                cancellable(ct, forward_stream(response, request, output)).await;
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

            fn call(self, mut request: HubInvocation, output: ResponseSink) -> Self::Future {
                Box::pin(async move {
                    $(
                        let $ty = FromInvocation::try_from_request(&mut request)?;
                    )+

                    tokio::spawn(async move {
                        let response = (self)($($ty,)+).await;
                        forward_single(response, request, output).await
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

            fn call_streaming(self, mut request: HubInvocation, output: ResponseSink) -> Self::Future {
                Box::pin(async move {
                    $(
                        let $ty = FromInvocation::try_from_request(&mut request)?;
                    )+

                    let ct = request.get_cancellation_token();
                    tokio::spawn(async move {
                        let response = (self)($($ty,)+).await;
                        cancellable(
                            ct,
                            forward_stream(response, request, output)
                        )
                        .await;
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

async fn cancellable(token: Option<CancellationToken>, fut: impl Future) {
    if let Some(token) = token {
        select! {
            _ = token.cancelled().fuse() => {},
            _ = fut.fuse() => {}
        }
    } else {
        fut.await;
    }
}
