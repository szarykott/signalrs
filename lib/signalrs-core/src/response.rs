use std::fmt::Debug;

use crate::{
    error::{InternalCommuncationError, SignalRError},
    extensions::StreamExtR,
    protocol::*,
};
use async_trait;
use flume::r#async::SendSink;
use futures::{
    sink::{Sink, SinkExt},
    stream::{Stream, StreamExt},
    Future, FutureExt,
};
use pin_project::pin_project;
use serde::Serialize;

const WEIRD_ENDING: &str = "\u{001E}";

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum HubResponseStruct {
    Text(String),
    Binary(Vec<u8>),
}

impl HubResponseStruct {
    pub fn unwrap_text(self) -> String {
        match self {
            HubResponseStruct::Text(v) => v,
            _ => panic!("cannot unwrap text"),
        }
    }
}

// ======== Function abstraction

#[allow(missing_debug_implementations)]
#[derive(Clone)]
pub struct ResponseSink {
    inner: SendSink<'static, HubResponseStruct>,
}

impl ResponseSink {
    pub fn new(sink: SendSink<'static, HubResponseStruct>) -> Self {
        ResponseSink { inner: sink }
    }
}

impl Sink<String> for ResponseSink {
    type Error = InternalCommuncationError;

    fn poll_ready(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready_unpin(cx).map_err(|e| e.into())
    }

    fn start_send(mut self: std::pin::Pin<&mut Self>, item: String) -> Result<(), Self::Error> {
        self.inner
            .start_send_unpin(HubResponseStruct::Text(item))
            .map_err(|e| e.into())
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_flush_unpin(cx).map_err(|e| e.into())
    }

    fn poll_close(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_close_unpin(cx).map_err(|e| e.into())
    }
}

#[async_trait::async_trait]
pub trait HubResponse {
    async fn forward(self, invocation_id: String, sink: ResponseSink) -> Result<(), SignalRError>;
}

#[async_trait::async_trait]
impl HubResponse for () {
    async fn forward(
        self,
        _invocation_id: String,
        _sink: ResponseSink,
    ) -> Result<(), SignalRError> {
        Ok(())
    }
}

macro_rules! impl_hub_response {
    ($($type:ty),+) => {
    $(
        #[async_trait::async_trait]
        impl HubResponse for $type
        {
            async fn forward(
                self,
                invocation_id: String,
                mut sink: ResponseSink,
            ) -> Result<(), SignalRError>
            {
                let completion = Completion::new(invocation_id, Some(self), None);
                let serialized = serde_json::to_string(&completion)? + WEIRD_ENDING;
                sink.send(serialized).await.map_err(|e| e.into())
            }
        }
    )*
    };
}

impl_hub_response!(usize, isize);
impl_hub_response!(i8, i16, i32, i64, i128);
impl_hub_response!(u8, u16, u32, u64, u128);
impl_hub_response!(f32, f64);
impl_hub_response!(String, &'static str);

#[async_trait::async_trait]
impl<R> HubResponse for Vec<R>
where
    R: HubResponse + Send + Serialize,
{
    async fn forward(
        self,
        invocation_id: String,
        mut sink: ResponseSink,
    ) -> Result<(), SignalRError> {
        let completion = Completion::new(invocation_id, Some(self), None);
        let text = serde_json::to_string(&completion)? + WEIRD_ENDING;
        sink.send(text).await.map_err(|e| e.into())
    }
}

#[async_trait::async_trait]
impl<R> HubResponse for Result<R, String>
// FIXME: String, rly?
where
    R: HubResponse + Send + Serialize,
{
    async fn forward(
        self,
        invocation_id: String,
        mut sink: ResponseSink,
    ) -> Result<(), SignalRError> {
        let completion = match self {
            Ok(ok) => Completion::new(invocation_id, Some(ok), None),
            Err(err) => Completion::new(invocation_id, None, Some(err)),
        };

        let text = serde_json::to_string(&completion)? + WEIRD_ENDING;

        sink.send(text).await.map_err(|e| e.into())
    }
}

// TODO: implement for Result and use to allow user to define their own faillible stream
pub trait Try {}

#[derive(Debug)]
pub struct HubStream;

#[derive(Debug)]
#[pin_project]
struct InfallibleHubStream<S> {
    #[pin]
    inner: S,
}

#[derive(Debug)]
struct FallibleHubStream<S>(S);

impl HubStream {
    pub fn infallible<S, I>(stream: S) -> impl HubResponse
    where
        S: Stream<Item = I> + Send + 'static,
        I: Send + Serialize,
    {
        InfallibleHubStream { inner: stream }
    }

    pub fn fallible<S, I>(stream: S) -> impl HubResponse
    where
        S: Stream<Item = Result<I, String>> + Send + 'static,
        I: Send + Serialize,
    {
        FallibleHubStream(stream)
    }
}

#[async_trait::async_trait]
impl<S, I> HubResponse for InfallibleHubStream<S>
where
    S: Stream<Item = I> + Send,
    I: Serialize + Send,
{
    async fn forward(self, invocation_id: String, sink: ResponseSink) -> Result<(), SignalRError> {
        let result = self.inner;

        let responses = result
            .zip(futures::stream::repeat(invocation_id.clone()))
            .map(|(e, id)| StreamItem::new(id, e))
            .map(|si| serde_json::to_string(&si).unwrap() + WEIRD_ENDING)
            .chain(futures::stream::once(async {
                let completion: Completion<usize> = Completion::new(invocation_id, None, None);
                serde_json::to_string(&completion).unwrap() + WEIRD_ENDING
            }));

        let mut responses = Box::pin(responses);
        let mut sink = Box::pin(sink);

        while let Some(item) = responses.next().await {
            sink.send(item).await?;
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl<S, I> HubResponse for FallibleHubStream<S>
where
    S: Stream<Item = Result<I, String>> + Send,
    I: Serialize + Send,
{
    async fn forward(self, invocation_id: String, sink: ResponseSink) -> Result<(), SignalRError> {
        let result = self.0;

        let responses = result
            .take_while_inclusive(|e| e.is_ok())
            .zip(futures::stream::repeat(invocation_id.clone()))
            .map(|(e, id)| -> Result<StreamItem<I>, Completion<()>> {
                match e {
                    Ok(item) => Ok(StreamItem::new(id, item)),
                    Err(e) => Err(Completion::<()>::new(id, None, Some(e))),
                }
            })
            .chain_if(
                |e| e.is_ok(),
                futures::stream::once(async {
                    let r: Result<StreamItem<I>, Completion<()>> =
                        Err(Completion::<()>::new(invocation_id, None, None));
                    r
                }),
            )
            .map(|e| match e {
                Ok(si) => serde_json::to_string(&si).unwrap() + WEIRD_ENDING,
                Err(cmp) => serde_json::to_string(&cmp).unwrap() + WEIRD_ENDING,
            });

        let mut responses = Box::pin(responses);
        let mut sink = Box::pin(sink);

        while let Some(item) = responses.next().await {
            sink.send(item).await?;
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct HubFutureWrapper<T>(pub T);

#[async_trait::async_trait]
impl<F, O> HubResponse for HubFutureWrapper<F>
where
    F: Future<Output = O> + Send,
    O: HubResponse + Send,
{
    async fn forward(self, invocation_id: String, sink: ResponseSink) -> Result<(), SignalRError> {
        let result = self.0.await;
        result.forward(invocation_id, sink).await
    }
}

// idea!
// pub trait HubResponse {
//     async fn prepare_stream(
//         self,
//         invocation_id: String,
//     ) -> impl Stream<Item = String>

//
// pub trait HubResponseExt : HubResponse {
//
// tu walnąć forward, wtedy HubResponse jest object safe o ile Stream będzie konkretną implementacją
//
//
//

pub trait HubResponseV2 {
    type Stream: Stream<Item = Self::Out>;
    type Out;

    fn into_stream(self) -> Self::Stream;
}

#[derive(Debug)]
pub struct ReadyStream<T>(futures::future::Ready<T>);

impl<T> Stream for ReadyStream<T> {
    type Item = T;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.0.poll_unpin(cx).map(|x| Some(x))
    }
}

macro_rules! impl_hub_response_v2 {
    ($($type:ty),+) => {
    $(
        impl HubResponseV2 for $type {
            type Stream = ReadyStream<Self::Out>;
            type Out = $type;

            fn into_stream(self) -> Self::Stream {
                ReadyStream(futures::future::ready(self))
            }
        }
    )*
    };
}

impl_hub_response_v2!(usize, isize);
impl_hub_response_v2!(i8, i16, i32, i64, i128);
impl_hub_response_v2!(u8, u16, u32, u64, u128);
impl_hub_response_v2!(f32, f64);
impl_hub_response_v2!(String, &'static str);

impl<Wrapped> Stream for InfallibleHubStream<Wrapped>
where
    Wrapped: Stream,
{
    type Item = <Wrapped as Stream>::Item;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let mut pinned = self.project();
        pinned.inner.poll_next_unpin(cx)
    }
}

impl<Wrapped> HubResponseV2 for InfallibleHubStream<Wrapped>
where
    Wrapped: Stream,
{
    type Stream = InfallibleHubStream<Wrapped>;
    type Out = <Wrapped as Stream>::Item;

    fn into_stream(self) -> Self::Stream {
        self
    }
}
