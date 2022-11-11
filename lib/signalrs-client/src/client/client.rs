use super::{
    builder::ClientBuilder, hub::Hub, messages::ClientMessage, ChannelSendError, SendBuilder,
    SignalRClientError,
};
use crate::protocol::{Completion, MessageType, StreamItem};
use flume::{r#async::RecvStream, Sender};
use futures::{stream::FuturesUnordered, Stream, StreamExt};
use serde::{de::DeserializeOwned, Deserialize};
use std::{
    collections::HashMap,
    marker::PhantomData,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tokio::task::JoinHandle;
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
    invocations: Arc<std::sync::Mutex<HashMap<String, Sender<ClientMessageWrapper>>>>,
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
    items: RecvStream<'a, ClientMessageWrapper>,
    invocation_id: String,
    client: &'a SignalRClient,
    upload: JoinHandle<Result<(), SignalRClientError>>,
    _phantom: PhantomData<T>,
}

pub(crate) struct ClientMessageWrapper {
    message_type: MessageType,
    message: ClientMessage,
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

    pub(crate) fn receive_messages(
        &self,
        messages: ClientMessage,
    ) -> Result<Command, SignalRClientError> {
        for message in messages.split() {
            // TODO: Add aggregate error subtype or log here
            // TODO: service close properly
            self.receive_message(message)?;
        }

        Ok(Command::None)
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
            if let Err(_) = sender.send(ClientMessageWrapper {
                message_type: MessageType::Completion,
                message,
            }) {
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
            let invocations = self.invocations.invocations.lock().unwrap(); // TODO: can it be posioned, use parking_lot?
            invocations
                .get(&invocation_id)
                .and_then(|sender| Some(sender.clone()))
        };

        if let Some(sender) = sender {
            if let Err(_) = sender.send(ClientMessageWrapper {
                message_type: MessageType::StreamItem,
                message,
            }) {
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
    pub fn builder(domain: impl ToString) -> ClientBuilder {
        ClientBuilder::new(domain)
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

    pub(crate) fn get_transport_handle(&self) -> Sender<ClientMessage> {
        self.transport_handle.clone()
    }

    pub(crate) async fn invoke<T>(
        &self,
        invocation_id: String,
        message: ClientMessage,
        streams: Vec<Box<dyn Stream<Item = ClientMessage> + Unpin + Send>>,
    ) -> Result<T, SignalRClientError>
    where
        T: DeserializeOwned,
    {
        let (tx, rx) = flume::bounded::<ClientMessageWrapper>(1);
        self.invocations
            .insert_invocation(invocation_id.to_owned(), tx);

        if let Err(error) = self.send_message(message).await {
            self.invocations.remove_invocation(&invocation_id);
            return Err(error);
        }

        let upload = tokio::spawn(Self::send_streams(self.transport_handle.clone(), streams));

        let result = rx.recv_async().await;
        upload.abort();

        event!(Level::DEBUG, "response received");

        self.invocations.remove_invocation(&invocation_id);

        let completion = result
            .map_err(|error| error.into())
            .and_then(|message| message.message.deserialize::<Completion<T>>())?;

        if completion.is_result() {
            Ok(completion.unwrap_result())
        } else if completion.is_error() {
            Err(SignalRClientError::InvocationError {
                message: completion.unwrap_error(),
            })
        } else {
            Err(SignalRClientError::ProtocolError {
                message: "completion without result".into(), // TODO: most probably this is allowed but need to compile now
            })
        }
    }

    pub(crate) async fn invoke_stream<'a, T>(
        &'a self,
        invocation_id: String,
        message: ClientMessage,
        streams: Vec<Box<dyn Stream<Item = ClientMessage> + Unpin + Send>>,
    ) -> Result<ResponseStream<'a, T>, SignalRClientError>
    where
        T: DeserializeOwned,
    {
        let (tx, rx) = flume::bounded::<ClientMessageWrapper>(100);
        self.invocations
            .insert_stream_invocation(invocation_id.to_owned(), tx);

        if let Err(error) = self.send_message(message).await {
            self.invocations.remove_stream_invocation(&invocation_id);
            return Err(error);
        }

        let handle = tokio::spawn(Self::send_streams(self.transport_handle.clone(), streams));

        let response_stream = ResponseStream {
            items: rx.into_stream(),
            invocation_id,
            client: &self,
            upload: handle,
            _phantom: Default::default(),
        };

        Ok(response_stream)
    }

    pub(crate) async fn send_message(
        &self,
        message: ClientMessage,
    ) -> Result<(), SignalRClientError> {
        self.transport_handle
            .send_async(message)
            .await
            .map_err(|e| -> ChannelSendError { e.into() })
            .map_err(|e| -> SignalRClientError { e.into() })?;

        event!(Level::DEBUG, "message sent");

        Ok(())
    }

    pub(crate) async fn send_streams(
        transport_handle: Sender<ClientMessage>,
        streams: Vec<Box<dyn Stream<Item = ClientMessage> + Unpin + Send>>,
    ) -> Result<(), SignalRClientError> {
        let mut futures = FuturesUnordered::new();
        for stream in streams.into_iter() {
            futures.push(Self::send_stream_internal(&transport_handle, stream));
        }

        while let Some(result) = futures.next().await {
            result?;
        }

        Ok(())
    }

    async fn send_stream_internal(
        transport_handle: &Sender<ClientMessage>,
        mut stream: Box<dyn Stream<Item = ClientMessage> + Unpin + Send>,
    ) -> Result<(), SignalRClientError> {
        while let Some(item) = stream.next().await {
            transport_handle
                .send_async(item)
                .await
                .map_err(|e| -> ChannelSendError { e.into() })
                .map_err(|e| -> SignalRClientError { e.into() })?;

            event!(Level::TRACE, "stream item sent");
        }

        event!(Level::DEBUG, "stream sent");

        Ok(())
    }
}

impl Invocations {
    fn insert_invocation(&self, id: String, sender: flume::Sender<ClientMessageWrapper>) {
        let mut invocations = self.invocations.lock().unwrap();
        (*invocations).insert(id, sender);
    }

    fn insert_stream_invocation(&self, id: String, sender: flume::Sender<ClientMessageWrapper>) {
        let mut invocations = self.invocations.lock().unwrap();
        (*invocations).insert(id, sender);
    }

    pub fn remove_invocation(&self, id: &String) -> Option<flume::Sender<ClientMessageWrapper>> {
        let mut invocations = self.invocations.lock().unwrap();
        (*invocations).remove(id)
    }

    pub fn remove_stream_invocation(&self, id: &String) {
        let mut invocations = self.invocations.lock().unwrap();
        (*invocations).remove(id);
    }
}

impl<'a, T> Drop for ResponseStream<'a, T> {
    fn drop(&mut self) {
        self.client
            .invocations
            .remove_stream_invocation(&self.invocation_id);

        self.upload.abort();
    }
}

// took this hack from: https://users.rust-lang.org/t/cannot-assign-to-data-in-a-dereference-of-pin-mut-myfutureimpl-t/70887
impl<'a, T> Unpin for ResponseStream<'a, T> {}

impl<'a, T> Stream for ResponseStream<'a, T>
where
    T: DeserializeOwned,
{
    type Item = Result<T, SignalRClientError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.items.poll_next_unpin(cx) {
            Poll::Ready(Some(message_wrapper)) => match message_wrapper.message_type {
                MessageType::StreamItem => {
                    let item = message_wrapper
                        .message
                        .deserialize::<StreamItem<T>>()
                        .and_then(|item| Ok(item.item));
                    Poll::Ready(Some(item))
                }
                MessageType::Completion => {
                    let deserialized = message_wrapper.message.deserialize::<Completion<T>>();

                    match deserialized {
                        Ok(completion) => {
                            if completion.is_error() {
                                error!(
                                    "invocation ended with error: {}",
                                    completion.unwrap_error()
                                );
                            }
                        }
                        Err(error) => error!("completion deserialization error: {}", error),
                    }

                    Poll::Ready(None)
                }
                _ => unreachable!(),
            },
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}
