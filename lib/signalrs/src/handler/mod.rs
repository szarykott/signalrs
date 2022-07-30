pub mod callable;

use futures::{select, Future, FutureExt};
use std::pin::Pin;
use tokio_util::sync::CancellationToken;

use crate::{
    error::SignalRError,
    extract::FromInvocation,
    invocation::HubInvocation,
    protocol::{Id, OptionalId},
    response::{HubResponse, ResponseSink},
};

use log::*;

pub trait Handler<T> {
    type Future: Future<Output = Result<(), SignalRError>> + Send;

    fn call(self, request: HubInvocation, output: ResponseSink, cancellable: bool) -> Self::Future;
}

impl<Fn, Fut, Ret> Handler<()> for Fn
where
    Fn: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = Ret> + Send,
    Ret: HubResponse + Send + 'static,
{
    type Future = Pin<Box<dyn Future<Output = Result<(), SignalRError>> + Send>>;

    fn call(self, request: HubInvocation, output: ResponseSink, cancellable: bool) -> Self::Future {
        Box::pin(async move {
            tokio::spawn(async move {
                let result = (self)().await;

                trace!("hub method call finished");

                if cancellable {
                    forward_cancellable(result, request, output).await
                } else {
                    forward_non_cancellable(result, request, output).await
                }
            });

            Ok(())
        })
    }
}

impl<Fn, Fut, Ret, T1> Handler<T1> for Fn
where
    Fn: FnOnce(T1) -> Fut + Send + 'static,
    Fut: Future<Output = Ret> + Send,
    Ret: HubResponse + Send + 'static,
    T1: FromInvocation + Send + 'static,
{
    type Future = Pin<Box<dyn Future<Output = Result<(), SignalRError>> + Send>>;

    fn call(
        self,
        mut request: HubInvocation,
        output: ResponseSink,
        cancellable: bool,
    ) -> Self::Future {
        Box::pin(async move {
            let t1 = FromInvocation::try_from_request(&mut request)?;

            trace!("extracted all arguments from request");

            tokio::spawn(async move {
                let result = (self)(t1).await;

                trace!("hub method call finished");

                if cancellable {
                    forward_cancellable(result, request, output).await
                } else {
                    forward_non_cancellable(result, request, output).await
                }
            });

            Ok(())
        })
    }
}

macro_rules! impl_handler {
    ($($ty:ident),+) => {
        #[allow(non_snake_case)]
        impl<Fn, Fut, Ret, $($ty,)+> Handler<($($ty,)+)> for Fn
        where
            Fn: FnOnce($($ty,)+) -> Fut + Send + 'static,
            Fut: Future<Output = Ret> + Send,
            Ret: HubResponse + Send + 'static,
            $(
                $ty: FromInvocation + Send + 'static,
            )+
        {
            type Future = Pin<Box<dyn Future<Output = Result<(), SignalRError>> + Send>>;

            fn call(
                self,
                mut request: HubInvocation,
                output: ResponseSink,
                cancellable: bool,
            ) -> Self::Future {
                Box::pin(async move {
                    $(
                        let $ty = FromInvocation::try_from_request(&mut request)?;
                    )+

                    trace!("extracted all arguments from request");

                    tokio::spawn(async move {
                        let result = (self)($($ty,)+).await;

                        trace!("hub method call finished");

                        if cancellable {
                            forward_cancellable(result, request, output).await
                        } else {
                            forward_non_cancellable(result, request, output).await
                        }
                    });

                    Ok(())

                })
            }
        }
    };
}

impl_handler!(T1, T2);
impl_handler!(T1, T2, T3);
impl_handler!(T1, T2, T3, T4);
impl_handler!(T1, T2, T3, T4, T5);
impl_handler!(T1, T2, T3, T4, T5, T6);
impl_handler!(T1, T2, T3, T4, T5, T6, T7);
impl_handler!(T1, T2, T3, T4, T5, T6, T7, T8);
impl_handler!(T1, T2, T3, T4, T5, T6, T7, T8, T9);
impl_handler!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
impl_handler!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
impl_handler!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);
impl_handler!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13);

async fn forward_non_cancellable<Ret: HubResponse>(
    result: Ret,
    request: HubInvocation,
    output: ResponseSink,
) -> Result<(), SignalRError> {
    let OptionalId { invocation_id } = serde_json::from_str(&request.unwrap_text())?;

    if let Some(id) = invocation_id {
        result.forward(id, output).await?;
    }

    Ok(())
}

async fn forward_cancellable<Ret: HubResponse + Send + 'static>(
    result: Ret,
    request: HubInvocation,
    output: ResponseSink,
) -> Result<(), SignalRError> {
    let Id { invocation_id } = serde_json::from_str(&request.unwrap_text())?;

    let cancellation_token = CancellationToken::new();

    let invocation_id1 = invocation_id.clone();
    let forward_future = async move {
        let result = result.forward(invocation_id1.clone(), output).await; // TODO: How to forward error?

        if let Err(e) = result {
            error!("error streaming hub method result: {}", e);
        } else {
            trace!("invocation forward succesfull")
        }
    };

    let cancellation_token1 = cancellation_token.clone();
    let fut = async move {
        select! {
            _ = forward_future.fuse() => {},
            _ = cancellation_token1.cancelled().fuse() => {}
        };
    };

    request
        .connection_state
        .inflight_invocations
        .insert(invocation_id, fut, cancellation_token);

    Ok(())
}
