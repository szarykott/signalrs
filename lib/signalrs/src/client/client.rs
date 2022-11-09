use super::{
    builder::ClientBuilder, hub::Hub, ChannelSendError, ClientMessage, SendBuilder,
    SignalRClientError,
};
use crate::protocol::MessageType;
use flume::Sender;
use futures::{stream::FuturesUnordered, Stream, StreamExt};
use serde::{de::DeserializeOwned, Deserialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::oneshot;
use tracing::*;

pub struct SignalRClient {
    invocations: Invocations,
    // hub: Option<Hub>,
    transport_handle: Sender<ClientMessage>,
}

pub(crate) struct TransportClientHandle {
    invocations: Invocations,
    hub: Option<Hub>,
}

#[derive(Default, Clone)]
pub(crate) struct Invocations {
    invocations: Arc<std::sync::Mutex<HashMap<String, oneshot::Sender<ClientMessage>>>>,
    stream_invocations: Arc<std::sync::Mutex<HashMap<String, Sender<ClientMessage>>>>,
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

pub struct ResponseStream<'a, T> {
    items: Box<dyn Stream<Item = Result<T, SignalRClientError>> + Unpin>,
    invocation_id: String,
    client: &'a SignalRClient,
}

pub(crate) fn new_client(
    transport_handle: Sender<ClientMessage>,
    hub: Option<Hub>,
) -> (TransportClientHandle, SignalRClient) {
    let invocations = Invocations::default();
    let transport_client_handle = TransportClientHandle::new(&invocations, hub);
    let client = SignalRClient::new(&invocations, transport_handle);

    (transport_client_handle, client)
}

impl TransportClientHandle {
    pub(crate) fn new(invocations: &Invocations, hub: Option<Hub>) -> Self {
        TransportClientHandle {
            invocations: invocations.to_owned(),
            hub,
        }
    }

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

        let sender = self.invocations.remove_invocation(&invocation_id);

        if let Some(sender) = sender {
            if let Err(_) = sender.send(message) {
                warn!("received completion for a dropped invocation");
                self.invocations.remove_invocation(&invocation_id);
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
            let invocations = self.invocations.stream_invocations.lock().unwrap(); // TODO: can it be posioned, use parking_lot?
            invocations
                .get(&invocation_id)
                .and_then(|sender| Some(sender.clone()))
        };

        if let Some(sender) = sender {
            if let Err(_) = sender.send(message) {
                warn!("received stream item for a dropped invocation");
                self.invocations.remove_stream_invocation(&invocation_id);
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
}

impl SignalRClient {
    pub fn builder(url: impl Into<String>) -> ClientBuilder {
        ClientBuilder::new(url)
    }

    pub fn call_builder<'a>(&'a self, method: impl ToString) -> SendBuilder<'a> {
        SendBuilder::new(self, method)
    }

    pub(crate) fn new(invocations: &Invocations, transport_handle: Sender<ClientMessage>) -> Self {
        SignalRClient {
            invocations: invocations.to_owned(),
            transport_handle,
        }
    }

    #[instrument(skip_all, name = "invoke")]
    pub(crate) async fn invoke<T>(
        &self,
        invocation_id: String,
        message: ClientMessage,
        streams: Vec<Box<dyn Stream<Item = Result<ClientMessage, SignalRClientError>> + Unpin>>,
    ) -> Result<T, SignalRClientError>
    where
        T: DeserializeOwned,
    {
        let (tx, rx) = oneshot::channel::<ClientMessage>();
        self.invocations
            .insert_invocation(invocation_id.to_owned(), tx);

        if let Err(error) = self.send_all(message, streams).await {
            self.invocations.remove_invocation(&invocation_id);
            return Err(error);
        }

        let result = rx
            .await
            .map_err(|error| error.into())
            .and_then(|message| message.deserialize());

        self.invocations.remove_invocation(&invocation_id);

        result
    }

    #[instrument(skip_all, name = "invoke stream")]
    pub(crate) async fn invoke_stream<'a, T>(
        &'a self,
        invocation_id: String,
        message: ClientMessage,
        streams: Vec<Box<dyn Stream<Item = Result<ClientMessage, SignalRClientError>> + Unpin>>,
    ) -> Result<ResponseStream<'a, T>, SignalRClientError>
    where
        T: DeserializeOwned,
    {
        let (tx, rx) = flume::bounded::<ClientMessage>(100);
        self.invocations
            .insert_stream_invocation(invocation_id.to_owned(), tx);

        if let Err(error) = self.send_all(message, streams).await {
            self.invocations.remove_stream_invocation(&invocation_id);
            return Err(error);
        }

        let response_stream = ResponseStream {
            items: Box::new(rx.into_stream().map(|item| item.deserialize::<T>())),
            invocation_id,
            client: &self,
        };

        Ok(response_stream)
    }

    async fn send_all(
        &self,
        message: ClientMessage,
        streams: Vec<Box<dyn Stream<Item = Result<ClientMessage, SignalRClientError>> + Unpin>>,
    ) -> Result<(), SignalRClientError> {
        self.send_message(message).await?;
        self.send_streams(streams).await
    }

    #[instrument(skip_all, name = "send message")]
    pub(crate) async fn send_message(
        &self,
        message: ClientMessage,
    ) -> Result<(), SignalRClientError> {
        self.transport_handle
            .send_async(message)
            .await
            .map_err(|e| -> ChannelSendError { e.into() })
            .map_err(|e| e.into())
    }

    #[instrument(skip_all, name = "send streams")]
    pub(crate) async fn send_streams(
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

    async fn send_stream_internal(
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

impl Invocations {
    fn insert_invocation(&self, id: String, sender: oneshot::Sender<ClientMessage>) {
        let mut invocations = self.invocations.lock().unwrap();
        (*invocations).insert(id, sender);
    }

    fn insert_stream_invocation(&self, id: String, sender: flume::Sender<ClientMessage>) {
        let mut invocations = self.stream_invocations.lock().unwrap();
        (*invocations).insert(id, sender);
    }

    pub fn remove_invocation(&self, id: &String) -> Option<oneshot::Sender<ClientMessage>> {
        let mut invocations = self.invocations.lock().unwrap();
        (*invocations).remove(id)
    }

    pub fn remove_stream_invocation(&self, id: &String) {
        let mut invocations = self.stream_invocations.lock().unwrap();
        (*invocations).remove(id);
    }
}

impl<'a, T> Drop for ResponseStream<'a, T> {
    fn drop(&mut self) {
        self.client
            .invocations
            .remove_stream_invocation(&self.invocation_id);
    }
}

impl<'a, T> Stream for ResponseStream<'a, T> {
    type Item = Result<T, SignalRClientError>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.items.poll_next_unpin(cx)
    }
}
