//! Arguments to an invocations
//!
//! In other words this module contains everyting that is necessary to
//! abstract over arguments in [`InvocationBuilder`](crate::invocation::InvocationBuilder)

use futures::{Stream, StreamExt};
use serde::Serialize;

/// Represents all possible arguments to a client invocation.
///
/// There are two kinds of arguments that client understands:
/// - simple arguments that will be serialized and passed to a server in one message
/// - [`Stream`](futures::stream::Stream)-compatible arguments that will be sent to server as soon as new items are available
/// # Example
/// ```rust, no_run
/// use signalrs_client::{SignalRClient, arguments::InvocationStream};
/// # use futures::StreamExt;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// #    let client = SignalRClient::builder("localhost")
/// #        .build()
/// #        .await?;
/// let stream = futures::stream::repeat("item").take(5);
///
/// client
///     .method("StreamEcho")
///     // server will receive it as a literal value
///     .arg("execute order 66")?
///     // server will receive it as a stream / async enumerable that it will be able to `.await`
///     .arg(InvocationStream::new(stream))?
///     .invoke_stream::<String>()
///     .await?;
/// # Ok(())
/// }
/// ```
pub enum InvocationArgs<T> {
    /// Simple argument that will be passed to server after serialization
    ///
    /// It is not intended to be created by hand. Client should be able to perform conversion for all availably types automatically.
    Argument(T),

    /// Stream argument that will be streamed asynchronously to a server
    ///
    /// *Needs to be constructed manually for the client.*
    Stream(InvocationStream<T>),
}

/// [`Stream`](futures::stream::Stream) wrapper for [`InvocationArgs`]
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
