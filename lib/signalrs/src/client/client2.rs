use super::{hub::Hub, ChannelSendError, ClientMessage, SignalRClientError};
use crate::protocol::MessageType;
use flume::Sender;
use futures::{stream::FuturesUnordered, Future, Stream, StreamExt};
use log::*;
use serde::Deserialize;
use std::{collections::HashMap, sync::Arc};

pub struct SignalRClient {
    invocations:
        Arc<std::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<ClientMessage>>>>,
    stream_invocations: Arc<std::sync::Mutex<HashMap<String, Sender<ClientMessage>>>>,
    hub: Option<Hub>,
    transport_handle: Sender<ClientMessage>,
}

#[derive(Deserialize)]
struct RoutingData {
    #[serde(rename = "invocationId")]
    pub invocation_id: Option<String>,
    #[serde(rename = "type")]
    pub message_type: MessageType,
}

pub enum Command {
    None,
    Close,
}

impl SignalRClient {
    pub(crate) fn receive_message(
        &self,
        message: ClientMessage,
    ) -> Result<Command, SignalRClientError> {
        let RoutingData {
            invocation_id,
            message_type,
        } = message.deserialize()?;

        return match message_type {
            MessageType::Invocation => self.receive_invocation(message),
            MessageType::Completion => self.receive_completion(invocation_id, message),
            MessageType::StreamItem => self.receive_stream_item(invocation_id, message),
            MessageType::Ping => self.receive_ping(),
            MessageType::Close => self.receive_close(),
            x => log_unsupported(x),
        };

        fn log_unsupported(message_type: MessageType) -> Result<Command, SignalRClientError> {
            warn!("received unsupported message type: {message_type}");
            Ok(Command::None)
        }
    }

    fn receive_invocation(&self, message: ClientMessage) -> Result<Command, SignalRClientError> {
        if let Some(hub) = &self.hub {
            hub.call(message)?;
        }

        Ok(Command::None)
    }

    fn receive_completion(
        &self,
        invocation_id: Option<String>,
        message: ClientMessage,
    ) -> Result<Command, SignalRClientError> {
        let invocation_id = invocation_id.ok_or_else(|| SignalRClientError::ProtocolError {
            message: "completion without invocation id".into(),
        })?;

        let sender = {
            let mut invocations = self.invocations.lock().unwrap(); // TODO: can it be posioned, use parking_lot?
            invocations.remove(&invocation_id)
        };

        if let Some(sender) = sender {
            if let Err(_) = sender.send(message) {
                warn!("received completion for a dropped invocation");
                self.remove_invocation(&invocation_id);
            }
        } else {
            warn!("received completion with unknown id");
        }

        Ok(Command::None)
    }

    fn receive_stream_item(
        &self,
        invocation_id: Option<String>,
        message: ClientMessage,
    ) -> Result<Command, SignalRClientError> {
        let invocation_id = invocation_id.ok_or_else(|| SignalRClientError::ProtocolError {
            message: "stream item without invocation id".into(),
        })?;

        let sender = {
            let invocations = self.stream_invocations.lock().unwrap(); // TODO: can it be posioned, use parking_lot?
            invocations
                .get(&invocation_id)
                .and_then(|sender| Some(sender.clone()))
        };

        if let Some(sender) = sender {
            if let Err(_) = sender.send(message) {
                warn!("received stream item for a dropped invocation");
                self.remove_stream_invocation(&invocation_id);
            }
        } else {
            warn!("received stream item with unknown id");
        }

        Ok(Command::None)
    }

    fn receive_ping(&self) -> Result<Command, SignalRClientError> {
        debug!("ping received");
        Ok(Command::None)
    }

    fn receive_close(&self) -> Result<Command, SignalRClientError> {
        info!("close received");
        Ok(Command::Close)
    }

    fn insert_invocation(&self, id: String, sender: tokio::sync::oneshot::Sender<ClientMessage>) {
        let mut invocations = self.invocations.lock().unwrap();
        (*invocations).insert(id, sender);
    }

    pub fn remove_invocation(&self, id: &String) {
        let mut invocations = self.invocations.lock().unwrap();
        (*invocations).remove(id);
    }

    pub fn remove_stream_invocation(&self, id: &String) {
        let mut invocations = self.stream_invocations.lock().unwrap();
        (*invocations).remove(id);
    }

    async fn send(self) -> Result<(), SignalRClientError> {
        todo!()
    }

    async fn invoke<T>(self) -> Result<T, SignalRClientError> {
        todo!()
    }

    // async fn invoke_stream<T>(
    //     self,
    // ) -> Result<impl Stream<Item = Result<T, SignalRClientError>>, SignalRClientError> {
    //     todo!()
    // }

    pub(crate) async fn send_message_internal(
        &self,
        message: ClientMessage,
    ) -> Result<(), SignalRClientError> {
        self.transport_handle
            .send_async(message)
            .await
            .map_err(|e| -> ChannelSendError { e.into() })
            .map_err(|e| e.into())
    }

    pub(crate) async fn send_streams_internal(
        &self,
        streams: Vec<Box<dyn Stream<Item = Result<ClientMessage, SignalRClientError>> + Unpin>>,
    ) -> Result<(), SignalRClientError> {
        let mut futures = FuturesUnordered::new();
        for stream in streams.into_iter() {
            futures.push(self.send_stream_internal(stream));
        }

        while let Some(result) = futures.next().await {
            result?;
        }

        Ok(())
    }

    pub(crate) async fn send_stream_internal(
        &self,
        mut stream: Box<dyn Stream<Item = Result<ClientMessage, SignalRClientError>> + Unpin>,
    ) -> Result<(), SignalRClientError> {
        while let Some(item) = stream.next().await {
            let item = item?;
            self.transport_handle
                .send_async(item)
                .await
                .map_err(|e| -> ChannelSendError { e.into() })
                .map_err(|e| -> SignalRClientError { e.into() })?;
        }

        Ok(())
    }
}
