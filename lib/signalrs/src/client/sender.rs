use crate::protocol::{Completion, StreamItem};
use futures::{
    sink::{Sink, SinkExt},
    stream::{FuturesUnordered, Stream, StreamExt},
};
use serde::Serialize;

use super::{
    messages::{ClientMessage, MessageEncoding},
    SignalRClientError,
};

pub struct SignalRClientSender<S> {
    pub(super) sink: S,
    pub(super) encoding: MessageEncoding,
}

impl<S> SignalRClientSender<S>
where
    S: Sink<ClientMessage, Error = SignalRClientError> + Unpin + Clone,
{
    pub(super) async fn actually_send(
        &mut self,
        serialized: ClientMessage,
        streams: Vec<(
            String,
            Box<dyn Stream<Item = Result<ClientMessage, SignalRClientError>> + Unpin>,
        )>,
    ) -> Result<(), SignalRClientError> {
        self.sink.send(serialized).await?;

        if !streams.is_empty() {
            let mut futures = FuturesUnordered::new();

            for (id, stream) in streams {
                let sink = self.sink.clone();
                let encoding = self.encoding;
                let future = async move {
                    match Self::stream_it(sink, id.as_str(), encoding, stream).await {
                        Ok(()) => Ok(()),
                        Err(error) => Err((id, error)),
                    }
                };

                futures.push(future);
            }

            while let Some(result) = futures.next().await {
                if let Err((id, error)) = result {
                    let completion = Completion::<()>::error(id, error.to_string());
                    let serialized = self.encoding.serialize(completion)?;
                    self.sink.send(serialized).await?;
                    return Err(error); // TODO: return here? client might still be interested in the rest of streams
                }
            }
        }

        Ok(())
    }

    async fn stream_it(
        mut sink: S,
        invocation_id: &str,
        encoding: MessageEncoding,
        mut stream: Box<dyn Stream<Item = Result<ClientMessage, SignalRClientError>> + Unpin>,
    ) -> Result<(), SignalRClientError> {
        loop {
            match stream.next().await {
                Some(Ok(value)) => {
                    let stream_item = StreamItem::new(invocation_id, value);
                    let serialized = encoding.serialize(stream_item)?;
                    sink.send(serialized).await?;
                }
                Some(Err(error)) => return Err(error),
                None => return Ok(()),
            }
        }
    }
}

pub enum InvocationPart<T> {
    Argument(T),
    Stream(InvocationStream<T>),
}

pub struct InvocationStream<T>(Box<dyn Stream<Item = T> + Unpin>);

impl<T> InvocationStream<T> {
    pub fn new(inner: impl Stream<Item = T> + Unpin + 'static) -> Self {
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
