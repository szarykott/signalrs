pub mod callable;

use std::pin::Pin;

use futures::Future;

use crate::{
    error::SignalRError,
    extract::FromInvocation,
    protocol::{Id, OptionalId},
    request::HubInvocation,
    response::{HubResponse, ResponseSink},
};

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
            if cancellable {
                let result = (self)().await;

                forward_cancellable(result, &request, output).await
            } else {
                let result = (self)().await;

                forward_non_cancellable(result, &request, output).await
            }
        })
    }
}

impl<Fn, Fut, Ret, T1> Handler<T1> for Fn
where
    Fn: FnOnce(T1) -> Fut + Send + 'static,
    Fut: Future<Output = Ret> + Send,
    Ret: HubResponse + Send + 'static,
    T1: FromInvocation + Send,
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

            let result = (self)(t1).await;

            if cancellable {
                forward_cancellable(result, &request, output).await
            } else {
                forward_non_cancellable(result, &request, output).await
            }
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
                $ty: FromInvocation + Send,
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

                    let result = (self)($($ty,)+).await;

                    if cancellable {
                        forward_cancellable(result, &request, output).await
                    } else {
                        forward_non_cancellable(result, &request, output).await
                    }
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
    request: &HubInvocation,
    output: ResponseSink,
) -> Result<(), SignalRError> {
    let invocation: OptionalId = serde_json::from_str(&request.unwrap_text())?;

    if let Some(id) = invocation.invocation_id {
        result.forward(id, output).await?;
    }

    Ok(())
}

async fn forward_cancellable<Ret: HubResponse + Send + 'static>(
    result: Ret,
    request: &HubInvocation,
    output: ResponseSink,
) -> Result<(), SignalRError> {
    let Id { invocation_id } = serde_json::from_str(&request.unwrap_text())?;

    let inflight_map = request.hub_state.inflight_invocations.clone();
    let inflight_map_clone = inflight_map.clone();
    let id_clone = invocation_id.clone();

    inflight_map
        .insert(invocation_id, async move {
            let _ = result.forward(id_clone.clone(), output).await; // TODO: How to forward error?
            inflight_map_clone.remove(&id_clone).await;
        })
        .await;

    Ok(())
}
