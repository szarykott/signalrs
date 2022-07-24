use flume::r#async::SendSink;
use futures::{Sink, SinkExt};

use crate::{connection::StreamItemPayload, error::InternalCommuncationError};

#[derive(Clone)]
pub struct ClientSink {
    pub(crate) sink: SendSink<'static, StreamItemPayload>,
}

impl Sink<StreamItemPayload> for ClientSink {
    type Error = InternalCommuncationError;

    fn poll_ready(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.sink.poll_ready_unpin(cx).map_err(|e| e.into())
    }

    fn start_send(
        mut self: std::pin::Pin<&mut Self>,
        item: StreamItemPayload,
    ) -> Result<(), Self::Error> {
        self.sink.start_send_unpin(item).map_err(|e| e.into())
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.sink.poll_flush_unpin(cx).map_err(|e| e.into())
    }

    fn poll_close(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.sink.poll_close_unpin(cx).map_err(|e| e.into())
    }
}
