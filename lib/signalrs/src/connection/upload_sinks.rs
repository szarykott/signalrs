use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use flume::r#async::SendSink;
use futures::{Sink, SinkExt};
use serde_json::Value;

use crate::server::error::InternalCommuncationError;

use super::StreamItemPayload;

#[derive(Clone, Default)]
pub struct UploadSinks {
    value: Arc<Mutex<HashMap<String, ClientSink>>>,
}

impl UploadSinks {
    pub fn insert(&self, stream_id: String, sink: ClientSink) {
        let mut mappings = self.value.lock().unwrap();
        (*mappings).insert(stream_id, sink);
    }

    pub fn get_sink(&self, stream_id: &String) -> Option<ClientSink> {
        let mappings = self.value.lock().unwrap();
        (*mappings).get(stream_id).and_then(|x| Some(x.clone()))
    }

    pub fn remove(&self, stream_id: &String) {
        let mut mappings = self.value.lock().unwrap();
        (*mappings).remove(stream_id);
    }
}

#[derive(Clone)]
pub struct ClientSink {
    pub(crate) sink: SendSink<'static, StreamItemPayload>,
}

impl Sink<Value> for ClientSink {
    type Error = InternalCommuncationError;

    fn poll_ready(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.sink.poll_ready_unpin(cx).map_err(|e| e.into())
    }

    fn start_send(mut self: std::pin::Pin<&mut Self>, item: Value) -> Result<(), Self::Error> {
        self.sink
            .start_send_unpin(StreamItemPayload::Text(item))
            .map_err(|e| e.into())
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
