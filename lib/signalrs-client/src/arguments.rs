//! Client invocation arguments

use futures::{Stream, StreamExt};
use serde::Serialize;

/// Client invocation arguments
///
/// Can be constructed from any type that implements `Serialize` or `InvocationStream` via `From` trait.
pub enum InvocationArgs<T> {
    /// Simple argument that will be passed to server after serialization
    Argument(T),

    /// Stream argument that will be streamed asynchronously to a server
    Stream(InvocationStream<T>),
}

/// Stream wrapper to be used in `args`
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

impl<T> From<T> for InvocationArgs<T>
where
    T: Serialize,
{
    fn from(object: T) -> Self {
        InvocationArgs::Argument(object)
    }
}

impl<T> From<InvocationStream<T>> for InvocationArgs<T>
where
    T: Serialize,
{
    fn from(stream: InvocationStream<T>) -> Self {
        InvocationArgs::Stream(stream)
    }
}
