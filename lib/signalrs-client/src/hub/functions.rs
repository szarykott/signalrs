use super::{
    error::HubError,
    invocation::{FromInvocation, HubInvocation},
};
use futures::Future;
use std::marker::PhantomData;

pub(crate) trait HubMethod {
    fn call(&self, request: HubInvocation) -> Result<(), HubError>;
}

/// Represents a hub method
///
/// It abstracts over method that can be called with HubInvocation as a sole argument.
pub trait Handler<T> {
    fn call(self, request: HubInvocation) -> Result<(), HubError>;
}

pub(crate) struct HandlerWrapper<H, T> {
    handler: H,
    _marker: PhantomData<T>,
}

impl<H, T> HubMethod for HandlerWrapper<H, T>
where
    H: Handler<T> + Clone,
{
    fn call(&self, request: HubInvocation) -> Result<(), HubError> {
        self.handler.clone().call(request)
    }
}

impl<H, T> From<H> for HandlerWrapper<H, T>
where
    H: Handler<T> + Clone,
{
    fn from(handler: H) -> Self {
        HandlerWrapper {
            handler,
            _marker: Default::default(),
        }
    }
}

impl<Fn, Fut> Handler<()> for Fn
where
    Fn: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send,
{
    fn call(self, _request: HubInvocation) -> Result<(), HubError> {
        tokio::spawn(async move {
            (self)().await;
        });

        Ok(())
    }
}

impl<Fn, Fut, T> Handler<T> for Fn
where
    Fn: FnOnce(T) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send,
    T: FromInvocation + Send + 'static,
{
    fn call(self, mut request: HubInvocation) -> Result<(), HubError> {
        let arg = T::try_from_invocation(&mut request)?;

        tokio::spawn(async move {
            (self)(arg).await;
        });

        Ok(())
    }
}

macro_rules! implement_handler {
    ($($ty:ident),+) => {
        #[allow(non_snake_case)]
        impl<Fn, Fut, $($ty,)+> Handler<($($ty,)+)> for Fn
        where
            Fn: FnOnce($($ty,)+) -> Fut + Send + 'static,
            Fut: Future<Output = ()> + Send,
            $(
                $ty: FromInvocation + Send + 'static,
            )+
        {
            fn call(self, mut request: HubInvocation) -> Result<(), HubError> {
                $(
                    let $ty = $ty::try_from_invocation(&mut request)?;
                )+

                tokio::spawn(async move {
                    (self)($($ty,)+).await;
                });

                Ok(())
            }
        }
    };
}

implement_handler!(T1, T2);
implement_handler!(T1, T2, T3);
implement_handler!(T1, T2, T3, T4);
implement_handler!(T1, T2, T3, T4, T5);
implement_handler!(T1, T2, T3, T4, T5, T6);
implement_handler!(T1, T2, T3, T4, T5, T6, T7);
implement_handler!(T1, T2, T3, T4, T5, T6, T7, T8);
implement_handler!(T1, T2, T3, T4, T5, T6, T7, T9, T10);
implement_handler!(T1, T2, T3, T4, T5, T6, T7, T9, T10, T11);
implement_handler!(T1, T2, T3, T4, T5, T6, T7, T9, T10, T11, T12);
implement_handler!(T1, T2, T3, T4, T5, T6, T7, T9, T10, T11, T12, T13);
