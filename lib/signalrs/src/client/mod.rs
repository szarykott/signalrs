mod builder;
mod client;
mod error;
mod hub;
mod messages;
mod send_builder;
mod websocket;

pub use self::{
    builder::ClientBuilder,
    client::SignalRClient,
    error::{ChannelSendError, SignalRClientError},
    messages::ClientMessage,
    messages::MessageEncoding,
    send_builder::SendBuilder,
};

use futures::{Stream, StreamExt};
use serde::Serialize;

pub enum InvocationPart<T> {
    Argument(T),
    Stream(InvocationStream<T>),
}

pub struct InvocationStream<T>(Box<dyn Stream<Item = T> + Unpin + Send>);

impl<T> InvocationStream<T> {
    pub fn new(inner: impl Stream<Item = T> + Unpin + Send + 'static) -> Self {
        InvocationStream(Box::new(inner))
    }
}

impl<T> Stream for InvocationStream<T> {
    type Item = T;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.0.poll_next_unpin(cx)
    }
}

pub trait IntoInvocationPart<T> {
    fn into(self) -> InvocationPart<T>;
}

impl<T> IntoInvocationPart<T> for T
where
    T: Serialize,
{
    fn into(self) -> InvocationPart<T> {
        InvocationPart::Argument(self)
    }
}

impl<T> IntoInvocationPart<T> for InvocationStream<T>
where
    T: Serialize,
{
    fn into(self) -> InvocationPart<T> {
        InvocationPart::Stream(self)
    }
}
