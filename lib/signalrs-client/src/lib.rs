//! # SignalR client
//!
//! SignalR is an open-source protocol that simplifies adding real-time web functionality to apps.
//! Real-time web functionality enables server-side code to push content to clients instantly.
//!
//! This library is an open source implementation of this protocol's client in Rust.
//! Originally developed at Microsoft in .NET ecosystem. Read more about it in [`offical documentation`].
//!
//! In technical terms it is a RPC framework with bidirectional streaming capabilities.
//!
//! ## Why SignalR
//!
//! ### Ergonomics
//!
//! It allows bidirectional communication with ergonimic programming model.
//! In cases where real-time communication is required it provides an easy to use framework, abstracting underlying transport layer.
//! Getting WebSockets right is not an easy task.
//!
//! ### Integration with existing services
//!
//! Since SignalR originated in .NET ecosystem, there are services that expose SignalR endpoints. This library allows easy integration with them.
//! This might be especially true for internal tooling at companies that do mostly C#. Truth to be told, it was a reason this library was created in the first place.
//!  
//! # Example
//!
//! ```rust, no_run
//! use signalrs_client::SignalRClient;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let client = SignalRClient::builder("localhost")
//!         .use_port(8080)
//!         .use_hub("echo")
//!         .build()
//!         .await?;
//!
//!     let result = client
//!         .method("echo")
//!         .arg("message to the server")?
//!         .invoke::<String>()
//!         .await?;
//!
//! # Ok(())
//! }
//! ```
//!
//! For more examples see examples folder in [`signalrs-client` examples].
//!
//! # Features of SignalR supported by `signalrs_client`
//!
//! SignalR as a protocol is defined by two documents:
//! - [HubProtcol]
//! - [TransportProtocol]
//!
//! Those documents describe details and full capabilities of the protocol.
//!
//! Unfortunately, `signalrs_client` only supports a subset of all features, especially regarding supported transports and message formats.
//! Hovewer, set of features supported is big enough for this library to be usable in simple scenarios.
//!
//! ## Known supported features
//!
//! - calling hub methods with all possible return types
//! - calling hub methods using both value and stream arguments
//! - client-side hub supports value arguments
//!
//! ### Not (yet) supported features
//!
//! - client-side hub with stream argumets
//!
//! ## Transport
//!
//! SignalR allows two types of transports to be used:
//! - WebSockets
//! - HTTP long polling + Server Sent Events
//!
//! This library only supports WebSockets now.
//!
//! ## Message encoding
//!
//! Two message encoding formats are allowed:
//! - JSON
//! - Message Pack
//!
//! This library only supports JSON now.
//!
//! [`offical documentation`]: https://learn.microsoft.com/en-us/aspnet/core/signalr/introduction?view=aspnetcore-7.0
//! [HubProtcol]: https://github.com/dotnet/aspnetcore/blob/main/src/SignalR/docs/specs/HubProtocol.md
//! [TransportProtocol]: https://github.com/dotnet/aspnetcore/blob/main/src/SignalR/docs/specs/TransportProtocols.md
//! [`signalrs-client` examples]: https://github.com/szarykott/signalrs/tree/main/lib/signalrs-client/examples

#![deny(unsafe_code)]

pub mod arguments;
pub mod builder;
pub mod error;
pub mod hub;
pub mod invocation;

mod messages;
mod protocol;
mod stream_ext;
mod transport;

use self::{error::ClientError, invocation::InvocationBuilder};
use crate::{
    builder::ClientBuilder,
    hub::Hub,
    messages::ClientMessage,
    protocol::{Completion, MessageType, StreamItem},
};
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
#[cfg(feature = "tokio-rt")]
use tokio::task::JoinHandle;
#[cfg(feature = "async-std-rt")]
use async_std::task::JoinHandle;
use tracing::*;

pub struct SignalRClient {
    invocations: Invocations,
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

pub(crate) enum Command {
    None,
    Close,
}

pub struct ResponseStream<'a, T> {
    items: RecvStream<'a, ClientMessageWrapper>,
    invocation_id: String,
    client: &'a SignalRClient,
    upload: JoinHandle<Result<(), ClientError>>,
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

    pub(crate) fn receive_messages(&self, messages: ClientMessage) -> Result<Command, ClientError> {
        for message in messages.split() {
            // TODO: Add aggregate error subtype or log here
            // TODO: service close properly
            self.receive_message(message)?;
        }

        Ok(Command::None)
    }

    pub(crate) fn receive_message(&self, message: ClientMessage) -> Result<Command, ClientError> {
        let RoutingData {
            invocation_id,
            message_type,
        } = message
            .deserialize()
            .map_err(ClientError::malformed_response)?;

        return match message_type {
            MessageType::Invocation => self.receive_invocation(message),
            MessageType::Completion => self.receive_completion(invocation_id, message),
            MessageType::StreamItem => self.receive_stream_item(invocation_id, message),
            MessageType::Ping => self.receive_ping(),
            MessageType::Close => self.receive_close(),
            x => log_unsupported(x),
        };

        fn log_unsupported(message_type: MessageType) -> Result<Command, ClientError> {
            warn!("received unsupported message type: {message_type}");
            Ok(Command::None)
        }
    }

    fn receive_invocation(&self, message: ClientMessage) -> Result<Command, ClientError> {
        if let Some(hub) = &self.hub {
            hub.call(message)?;
        }

        Ok(Command::None)
    }

    fn receive_completion(
        &self,
        invocation_id: Option<String>,
        message: ClientMessage,
    ) -> Result<Command, ClientError> {
        let invocation_id = invocation_id.ok_or_else(|| {
            ClientError::protocol_violation("received completion without invocation id")
        })?;

        let sender = self.invocations.remove_invocation(&invocation_id);

        if let Some(sender) = sender {
            if sender
                .send(ClientMessageWrapper {
                    message_type: MessageType::Completion,
                    message,
                })
                .is_err()
            {
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
    ) -> Result<Command, ClientError> {
        let invocation_id = invocation_id.ok_or_else(|| {
            ClientError::protocol_violation("received stream item without stream id")
        })?;

        let sender = {
            let invocations = self.invocations.invocations.lock().unwrap(); // TODO: can it be posioned, use parking_lot?
            invocations.get(&invocation_id).cloned()
        };

        if let Some(sender) = sender {
            if sender
                .send(ClientMessageWrapper {
                    message_type: MessageType::StreamItem,
                    message,
                })
                .is_err()
            {
                warn!("received stream item for a dropped invocation");
                self.invocations.remove_stream_invocation(&invocation_id);
            }
        } else {
            warn!("received stream item with unknown id");
        }

        Ok(Command::None)
    }

    fn receive_ping(&self) -> Result<Command, ClientError> {
        debug!("ping received");
        Ok(Command::None)
    }

    fn receive_close(&self) -> Result<Command, ClientError> {
        info!("close received");
        Ok(Command::Close)
    }
}

impl SignalRClient {
    pub fn builder(domain: impl ToString) -> ClientBuilder {
        ClientBuilder::new(domain)
    }

    pub fn method(&self, method: impl ToString) -> InvocationBuilder<'_> {
        InvocationBuilder::new(self, method)
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

    pub(crate) async fn invoke_option<T>(
        &self,
        invocation_id: String,
        message: ClientMessage,
        streams: Vec<Box<dyn Stream<Item = ClientMessage> + Unpin + Send>>,
    ) -> Result<Option<T>, ClientError>
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

        #[cfg(feature = "tokio-rt")]
        let upload = tokio::spawn(Self::send_streams(self.transport_handle.clone(), streams));
        #[cfg(feature = "async-std-rt")]
        let upload = async_std::task::spawn(Self::send_streams(self.transport_handle.clone(), streams));

        let result = rx.recv_async().await;
        #[cfg(feature = "tokio-rt")]
        upload.abort();
        #[cfg(feature = "async-std-rt")]
        upload.cancel();

        self.invocations.remove_invocation(&invocation_id);

        let completion = result
            .map_err(ClientError::no_response)
            .and_then(|message| {
                message
                    .message
                    .deserialize::<Completion<T>>()
                    .map_err(ClientError::malformed_response)
            })?;

        event!(Level::DEBUG, "response received");

        if completion.is_result() {
            Ok(Some(completion.unwrap_result()))
        } else if completion.is_error() {
            Err(ClientError::result(completion.unwrap_error()))
        } else {
            Ok(None)
        }
    }

    pub(crate) async fn invoke_stream<T>(
        &self,
        invocation_id: String,
        message: ClientMessage,
        streams: Vec<Box<dyn Stream<Item = ClientMessage> + Unpin + Send>>,
    ) -> Result<ResponseStream<'_, T>, ClientError>
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

        #[cfg(feature = "tokio-rt")]
        let handle = tokio::spawn(Self::send_streams(self.transport_handle.clone(), streams));
        #[cfg(feature = "async-std-rt")]
        let handle = async_std::task::spawn(Self::send_streams(self.transport_handle.clone(), streams));

        let response_stream = ResponseStream {
            items: rx.into_stream(),
            invocation_id,
            client: self,
            upload: handle,
            _phantom: Default::default(),
        };

        Ok(response_stream)
    }

    pub(crate) async fn send_message(&self, message: ClientMessage) -> Result<(), ClientError> {
        self.transport_handle
            .send_async(message)
            .await
            .map_err(ClientError::transport)?;

        event!(Level::DEBUG, "message sent");

        Ok(())
    }

    pub(crate) async fn send_streams(
        transport_handle: Sender<ClientMessage>,
        streams: Vec<Box<dyn Stream<Item = ClientMessage> + Unpin + Send>>,
    ) -> Result<(), ClientError> {
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
    ) -> Result<(), ClientError> {
        while let Some(item) = stream.next().await {
            transport_handle
                .send_async(item)
                .await
                .map_err(ClientError::transport)?;

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

        #[cfg(feature = "tokio-rt")]
        self.upload.abort();
        #[cfg(feature = "async-std-rt")]
        self.upload.cancel();
    }
}

// took this hack from: https://users.rust-lang.org/t/cannot-assign-to-data-in-a-dereference-of-pin-mut-myfutureimpl-t/70887
impl<'a, T> Unpin for ResponseStream<'a, T> {}

impl<'a, T> Stream for ResponseStream<'a, T>
where
    T: DeserializeOwned,
{
    type Item = Result<T, ClientError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.items.poll_next_unpin(cx) {
            Poll::Ready(Some(message_wrapper)) => match message_wrapper.message_type {
                MessageType::StreamItem => {
                    let item = message_wrapper
                        .message
                        .deserialize::<StreamItem<T>>()
                        .map_err(ClientError::malformed_response)
                        .map(|item| item.item);
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
