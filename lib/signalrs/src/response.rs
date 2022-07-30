use std::{fmt::Debug, pin::Pin};

use crate::{
    error::{InternalCommuncationError, SignalRError},
    extensions::StreamExtR,
    invocation::HubInvocation,
    protocol::*,
    serialization,
};
use async_trait;
use flume::r#async::SendSink;
use futures::{
    pin_mut,
    sink::{Sink, SinkExt},
    stream::{Stream, StreamExt},
    Future, FutureExt,
};
use pin_project::pin_project;
use serde::Serialize;
use tokio_util::sync::CancellationToken;

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
                let serialized = serialization::to_json(&completion)?;
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
        let text = serialization::to_json(&completion)?;
        sink.send(text).await.map_err(|e| e.into())
    }
}

#[async_trait::async_trait]
impl<R, E> HubResponse for Result<R, E>
where
    R: HubResponse + Send + Serialize,
    E: ToString + Send,
{
    async fn forward(
        self,
        invocation_id: String,
        mut sink: ResponseSink,
    ) -> Result<(), SignalRError> {
        let completion = match self {
            Ok(ok) => Completion::new(invocation_id, Some(ok), None),
            Err(err) => Completion::new(invocation_id, None, Some(err.to_string())),
        };

        let text = serialization::to_json(&completion)?;

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
            .map(|si| serialization::to_json(&si).unwrap())
            .chain(futures::stream::once(async {
                let completion: Completion<usize> = Completion::new(invocation_id, None, None);
                serialization::to_json(&completion).unwrap()
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
                Ok(si) => serialization::to_json(&si).unwrap(),
                Err(cmp) => serialization::to_json(&cmp).unwrap(),
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

// =========================================== //

pub trait IntoResponse {
    type Out: Serialize + Send;

    /// Specifies if this item carries an error variant
    ///
    /// If so, this item will break the response stream with error message sent to the client.
    fn is_error(&self) -> bool {
        false
    }
    fn into_completion(self, invocation_id: String) -> Completion<Self::Out>;
    fn into_stream_item(self, invocation_id: String) -> StreamItem<Self::Out>;
}

macro_rules! impl_into_response {
    ($($type:ty),+) => {
        $(
            impl IntoResponse for $type {
                type Out = Self;

                fn into_completion(self, invocation_id: String) -> Completion<Self::Out> {
                    Completion::result(invocation_id, self)
                }

                fn into_stream_item(self, invocation_id: String) -> StreamItem<Self::Out> {
                    StreamItem::new(invocation_id, self)
                }
            }
        )+
    };
}

impl_into_response!(());
impl_into_response!(usize, isize);
impl_into_response!(i8, i16, i32, i64, i128);
impl_into_response!(u8, u16, u32, u64, u128);
impl_into_response!(f32, f64);
impl_into_response!(String, &'static str);

impl<T, E> IntoResponse for Result<T, E>
where
    T: Serialize + Send,
    E: Into<String> + Debug,
{
    type Out = T;

    fn is_error(&self) -> bool {
        self.is_err()
    }

    fn into_completion(self, invocation_id: String) -> Completion<Self::Out> {
        match self {
            Ok(result) => Completion::result(invocation_id, result),
            Err(error) => Completion::error(invocation_id, error),
        }
    }

    fn into_stream_item(self, invocation_id: String) -> StreamItem<Self::Out> {
        let value = self.expect("stream item should never be constructed from error");
        StreamItem::new(invocation_id, value)
    }
}

pub trait IntoHubStream {
    type Stream: Stream<Item = Self::Out> + Send;
    type Out: IntoResponse + Send;
    fn into_stream(self) -> Self::Stream;
}

impl<T> IntoHubStream for T
where
    T: Stream + Send,
    <T as Stream>::Item: IntoResponse + Send,
{
    type Stream = T;
    type Out = <T as Stream>::Item;

    fn into_stream(self) -> Self::Stream {
        self
    }
}

pub async fn forward_single<Res>(
    msg: Res,
    invocation: HubInvocation,
    mut sink: ResponseSink,
) -> Result<(), SignalRError>
where
    Res: IntoResponse,
    <Res as IntoResponse>::Out: Serialize,
{
    let OptionalId { invocation_id } = serde_json::from_str(&invocation.unwrap_text())?;
    if let Some(invocation_id) = invocation_id {
        let completion = msg.into_completion(invocation_id);
        let json = serde_json::to_string(&completion)?;
        sink.send(json).await?;
    }

    Ok(())
}

pub async fn forward_stream<Res>(
    stream: Res,
    invocation: HubInvocation,
    mut sink: ResponseSink,
) -> Result<(), SignalRError>
where
    Res: IntoHubStream,
{
    let Id { invocation_id } = serde_json::from_str(&invocation.unwrap_text())?;

    let stream = stream.into_stream();

    pin_mut!(stream);

    while let Some(item) = stream.next().await {
        if item.is_error() {
            let completion = item.into_completion(invocation_id.clone());
            let json = serde_json::to_string(&completion)?;
            sink.send(json).await?;
            return Ok(()); // expected error
        }

        let stream_item = item.into_stream_item(invocation_id.clone());
        let json = serde_json::to_string(&stream_item)?;
        sink.send(json).await?;
    }

    let completion = Completion::<()>::ok(invocation_id);
    let json = serde_json::to_string(&completion)?;
    sink.send(json).await?;

    Ok(())
}
