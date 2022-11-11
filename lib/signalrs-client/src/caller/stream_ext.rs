use crate::{messages::SerializationError, protocol::Completion};

use super::messages::{ClientMessage, MessageEncoding};
use futures::{Stream, StreamExt};
use std::{
    pin::Pin,
    task::{Context, Poll},
};
use tracing::*;

pub(crate) trait SignalRStreamExt: Sized {
    fn append_completion(
        self,
        stream_id: String,
        encoding: MessageEncoding,
    ) -> AppendCompletion<Self>;
}

impl<T> SignalRStreamExt for T
where
    T: Stream,
{
    fn append_completion(
        self,
        stream_id: String,
        encoding: MessageEncoding,
    ) -> AppendCompletion<Self> {
        AppendCompletion {
            stream_id,
            encoding,
            inner: self,
            finished: false,
        }
    }
}

pub(crate) struct AppendCompletion<S> {
    stream_id: String,
    encoding: MessageEncoding,
    inner: S,
    finished: bool,
}

impl<S> AppendCompletion<S> {
    fn get_ok_completion(&self) -> ClientMessage {
        let completion = Completion::<()>::ok(self.stream_id.clone());
        let serialized = self.encoding.serialize(completion).unwrap_or_else(|error| {
            event!(Level::ERROR, error = error.to_string(), "serialization error");
            self.get_infallible_completion()
        });

        serialized
    }

    fn get_error_completion(&self, error: String) -> ClientMessage {
        let completion = Completion::<()>::error(self.stream_id.clone(), error);
        let serialized = self.encoding.serialize(completion).unwrap_or_else(|error| {
            event!(Level::ERROR, error = error.to_string(), "serialization error");
            self.get_infallible_completion()
        });

        serialized
    }

    fn get_infallible_completion(&self) -> ClientMessage {
        let completion = Completion::<()>::error(self.stream_id.clone(), "error in a stream");
        self.encoding.serialize(completion).unwrap()
    }
}

impl<S> Stream for AppendCompletion<S>
where
    S: Stream<Item = Result<ClientMessage, SerializationError>> + Unpin,
{
    type Item = ClientMessage;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.finished {
            return Poll::Ready(None);
        }

        match self.inner.poll_next_unpin(cx) {
            Poll::Ready(Some(Ok(message))) => Poll::Ready(Some(message)),
            Poll::Ready(Some(Err(error))) => {
                let error = error.to_string();
                event!(Level::ERROR, error, "error in stream");
                let serialized = self.get_error_completion(error);
                self.finished = true;
                Poll::Ready(Some(serialized))
            }
            Poll::Ready(None) => {
                let completion = self.get_ok_completion();
                self.finished = true;
                Poll::Ready(Some(completion))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
