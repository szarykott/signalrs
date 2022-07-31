use crate::{error::InternalCommuncationError, protocol::*};
use flume::r#async::SendSink;
use futures::{
    sink::{Sink, SinkExt},
    stream::Stream,
};
use serde::Serialize;
use std::fmt::Debug;

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

impl<T> IntoResponse for Option<T>
where
    T: Serialize + Send + Default,
{
    type Out = Self;

    fn into_completion(self, invocation_id: String) -> Completion<Self::Out> {
        Completion::result(invocation_id, self)
    }

    fn into_stream_item(self, invocation_id: String) -> StreamItem<Self::Out> {
        StreamItem::new(invocation_id, self)
    }
}

impl<T> IntoResponse for Vec<T>
where
    T: Serialize + Send + Default,
{
    type Out = Self;

    fn into_completion(self, invocation_id: String) -> Completion<Self::Out> {
        Completion::result(invocation_id, self)
    }

    fn into_stream_item(self, invocation_id: String) -> StreamItem<Self::Out> {
        StreamItem::new(invocation_id, self)
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
