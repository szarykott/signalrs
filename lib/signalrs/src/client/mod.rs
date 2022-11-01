mod builder;
mod error;
mod hub;
mod messages;
mod receiver;
mod sender;

use self::hub::Hub;
pub use self::{
    error::{ChannelSendError, SignalRClientError},
    messages::ClientMessage,
    messages::MessageEncoding,
    receiver::SignalRClientReceiver,
    sender::{IntoInvocationPart, InvocationPart, SignalRClientSender},
};
use crate::protocol::{Invocation, StreamInvocation};
use futures::{Sink, Stream, StreamExt};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

pub struct SignalRClient<Sink, Stream> {
    sender: sender::SignalRClientSender<Sink>,
    receiver: receiver::SignalRClientReceiver<Stream>,
    encoding: MessageEncoding,
}

pub fn new_text_client<Out, In>(output: Out, input: In, hub: Option<Hub>) -> SignalRClient<Out, In>
where
    Out: Sink<ClientMessage, Error = SignalRClientError> + Unpin,
    In: Stream<Item = ClientMessage> + Send + Unpin + 'static,
{
    let mut receiver = SignalRClientReceiver {
        invocations: Arc::new(Mutex::new(HashMap::new())),
        incoming_messages: Some(input),
        encoding: MessageEncoding::Json,
        hub,
    };

    receiver.start_receiver_loop();

    SignalRClient {
        sender: SignalRClientSender {
            sink: output,
            encoding: MessageEncoding::Json,
        },
        receiver,
        encoding: MessageEncoding::Json,
    }
}

impl<Si, St> SignalRClient<Si, St>
where
    Si: Sink<ClientMessage, Error = SignalRClientError> + Unpin + Clone,
    St: Stream<Item = ClientMessage> + Send + Unpin + 'static,
{
    /// Creates method invocation builder with a specified named hub method
    pub fn method<'a>(&'a mut self, target: impl ToString) -> SignalRSendBuilder<'a, Si, St> {
        SignalRSendBuilder {
            sender: &mut self.sender,
            receiver: &mut self.receiver,
            encoding: self.encoding,
            state: Arguments {
                method: target.to_string(),
                arguments: Default::default(),
                streams: Default::default(),
            },
        }
    }
}

pub struct SignalRSendBuilder<'a, Si, St> {
    sender: &'a mut sender::SignalRClientSender<Si>,
    receiver: &'a mut receiver::SignalRClientReceiver<St>,
    encoding: MessageEncoding,
    state: Arguments,
}

pub struct Arguments {
    method: String,
    arguments: Vec<serde_json::Value>,
    streams: Vec<Box<dyn Stream<Item = Result<ClientMessage, SignalRClientError>> + Unpin>>,
}

impl<'a, Si, St> SignalRSendBuilder<'a, Si, St>
where
    Si: Sink<ClientMessage, Error = SignalRClientError> + Unpin + Clone,
    St: Stream<Item = ClientMessage> + Send + Unpin + 'static,
{
    /// Adds ordered argument to invocation
    pub fn arg<A>(mut self, arg: A) -> Result<Self, SignalRClientError>
    where
        A: IntoInvocationPart<A> + Serialize + 'static,
    {
        match arg.into() {
            InvocationPart::Argument(arg) => self.state.arguments.push(serde_json::to_value(arg)?),
            InvocationPart::Stream(stream) => {
                let encoding = self.encoding;
                self.state.streams.push(Box::new(
                    stream.map(move |x| encoding.serialize(x).map_err(|x| x.into())),
                )
                    as Box<dyn Stream<Item = Result<ClientMessage, SignalRClientError>> + Unpin>);
            }
        };

        Ok(self)
    }

    /// Invokes a hub method on the server without waiting for a response
    pub async fn send(self) -> Result<(), SignalRClientError> {
        let mut invocation = Invocation::new_non_blocking(
            self.state.method,
            Self::args_as_option(self.state.arguments),
        );

        let stream_ids = Self::get_stream_ids(self.state.streams.len());
        invocation.with_streams(stream_ids.clone());

        let serialized = self.encoding.serialize(&invocation)?;

        self.sender
            .send(serialized, stream_ids.into_iter().zip(self.state.streams).collect())
            .await
    }

    /// Invokes a hub method on the server expecting a single response
    pub async fn invoke<T>(self) -> Result<T, SignalRClientError>
    where
        T: DeserializeOwned,
    {
        let mut invocation = Invocation::new_non_blocking(
            self.state.method,
            Self::args_as_option(self.state.arguments),
        );

        let invocation_id = Uuid::new_v4().to_string();
        invocation.add_invocation_id(invocation_id.clone());

        let stream_ids = Self::get_stream_ids(self.state.streams.len());
        invocation.with_streams(stream_ids.clone());

        let serialized = self.encoding.serialize(&invocation)?;

        let rx = self.receiver.setup_receive_once(&invocation_id);

        let send_result = self
            .sender
            .send(serialized, stream_ids.into_iter().zip(self.state.streams).collect())
            .await;

        if let e @ Err(_) = send_result {
            self.receiver.remove_invocation(&invocation_id);
            e?;
        }

        self.receiver
            .receive_once::<T>(invocation_id, rx)
            .await?
            .map_err(|x| SignalRClientError::ProtocolError {
                message: x.to_owned(), // FIXME: Error!
            })
    }

    /// Invokes a hub method on the server expecting a stream response
    pub async fn invoke_stream<T>(
        self,
    ) -> Result<impl Stream<Item = Result<T, SignalRClientError>>, SignalRClientError>
    where
        T: DeserializeOwned + Send + 'static,
    {
        let invocation_id = Uuid::new_v4().to_string();

        let mut invocation = StreamInvocation::new(
            invocation_id.clone(),
            self.state.method,
            Self::args_as_option(self.state.arguments),
        );

        let stream_ids = Self::get_stream_ids(self.state.streams.len());
        invocation.with_streams(stream_ids.clone());

        let serialized = self.encoding.serialize(&invocation)?;

        let rx = self.receiver.setup_receive_stream(invocation_id.clone());

        let result = self
            .sender
            .send(serialized, stream_ids.into_iter().zip(self.state.streams).collect())
            .await;

        if let e @ Err(_) = result {
            self.receiver.remove_invocation(&invocation_id);
            e?;
        }

        self.receiver.receive_stream::<T>(invocation_id, rx).await
    }

    fn args_as_option(arguments: Vec<serde_json::Value>) -> Option<Vec<serde_json::Value>> {
        if arguments.is_empty() {
            None
        } else {
            Some(arguments)
        }
    }

    fn get_stream_ids(num_streams: usize) -> Vec<String> {
        let mut stream_ids = Vec::new();
        if num_streams > 0 {
            for _ in 0..num_streams {
                stream_ids.push(Uuid::new_v4().to_string());
            }
        }

        stream_ids
    }
}
