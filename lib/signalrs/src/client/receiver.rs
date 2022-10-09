use flume::{Receiver, Sender};
use futures::{Stream, StreamExt};
use log::*;
use serde::{de::DeserializeOwned, Deserialize};
use std::collections::HashSet;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use super::{
    messages::{ClientMessage, MessageEncoding},
    ChannelSendError, SignalRClientError,
};
use crate::protocol::{Completion, MessageType, StreamItem};

pub struct SignalRClientReceiver<S, I> {
    pub(super) incoming_messages: Option<S>,
    pub(super) invocations: Arc<Mutex<HashMap<String, Sender<I>>>>,
    pub(super) encoding: MessageEncoding,
}

impl<S> SignalRClientReceiver<S, ClientMessage>
where
    S: Stream<Item = ClientMessage> + Send + Unpin + 'static,
{
    pub fn setup_receive_once(&self, invocation_id: impl ToString) -> Receiver<ClientMessage> {
        let (tx, rx) = flume::bounded::<ClientMessage>(1);
        self.insert_invocation(invocation_id.to_string(), tx);
        rx
    }

    pub async fn receive_once<T: DeserializeOwned>(
        &self,
        invocation_id: impl ToString,
        rx: Receiver<ClientMessage>,
    ) -> Result<Result<T, String>, SignalRClientError> {
        let result = Self::receive_completion::<T>(rx).await;
        self.remove_invocation(&invocation_id.to_string());
        result
    }

    async fn receive_completion<T: DeserializeOwned>(
        rx: Receiver<ClientMessage>,
    ) -> Result<Result<T, String>, SignalRClientError> {
        let next = rx.recv_async().await?;
        let completion: Completion<T> = next.deserialize()?;

        if completion.is_result() {
            return Ok(Ok(completion.unwrap_result()));
        }

        if completion.is_error() {
            return Ok(Err(completion.unwrap_error()));
        }

        // FIXME: probably this is allowed if client invokes blocking method with ()/void return type

        Err(SignalRClientError::ProtocolError {
            message: "Callee completed invocation without success or error indicator".into(),
        })
    }

    pub fn setup_receive_stream(&self, invocation_id: impl ToString) -> Receiver<ClientMessage> {
        let (tx, rx) = flume::bounded::<ClientMessage>(100);
        self.insert_invocation(invocation_id.to_string(), tx);
        rx
    }

    pub async fn receive_stream<T: DeserializeOwned + Send + 'static>(
        &self,
        invocation_id: impl Into<String>,
        receiver: Receiver<ClientMessage>,
    ) -> Result<ReceiveStream<Result<T, SignalRClientError>, ClientMessage>, SignalRClientError>
    {
        let (mut tx, rx) = flume::bounded::<Result<T, SignalRClientError>>(100);

        let future = async move {
            let mut input_stream = receiver.into_stream();
            while let Some(next) = input_stream.next().await {
                let message_type = match next.deserialize() {
                    Ok(MessageTypeWrapper { message_type }) => message_type,
                    Err(e) => {
                        error!("{}", e); // FIXME: Forward
                        break;
                    }
                };
                match message_type {
                    MessageType::StreamItem => {
                        let stream_item: StreamItem<T> = match next.deserialize() {
                            Ok(item) => item,
                            Err(e) => {
                                error!("{}", e); // FIXME: Forward
                                break;
                            }
                        };

                        if let Err(e) = tx.send_async(Ok(stream_item.item)).await {
                            error!("{}", e); // FIXME: Forward
                            break;
                        }
                    }
                    MessageType::Completion => {
                        let completion: Completion<()> = match next.deserialize() {
                            Ok(item) => item,
                            Err(e) => {
                                error!("{}", e); // FIXME: Forward
                                break;
                            }
                        };

                        if completion.is_error() {
                            let error = completion.unwrap_error();
                            if let Err(e) = tx
                                .send_async(Err(SignalRClientError::InvocationError {
                                    message: error,
                                }))
                                .await
                            {
                                error!("{}", e); // FIXME: Forward
                            }

                            break;
                        }

                        break;
                    }
                    message_type => {
                        send_unsupported_error(&mut tx, message_type).await;
                        break;
                    }
                }
            }
        };

        tokio::spawn(future);

        return Ok(ReceiveStream {
            inner: Box::new(rx.into_stream()),
            invocation_id: invocation_id.into(),
            invocations: self.invocations.clone(),
        });

        async fn send_unsupported_error<T: Send + 'static>(
            tx: &mut Sender<Result<T, SignalRClientError>>,
            message_type: MessageType,
        ) {
            tx.send_async(Err(SignalRClientError::ProtocolError {
                message: format!("Received illegal {}", message_type),
            }))
            .await
            .unwrap_or_else(|e| error!("{}", e));
        }
    }

    pub(super) fn start_receiver_loop(&mut self) {
        let mut incoming = self
            .incoming_messages
            .take()
            .expect("Improper receiver initialization");

        let invocations = self.invocations.clone();
        let encoding = self.encoding;

        let future = async move {
            while let Some(next) = incoming.next().await {
                match next.deserialize::<MaybeInvocationId>() {
                    Ok(MaybeInvocationId { invocation_id }) => {
                        if let Some(invocation_id) = invocation_id {
                            if let Err(error) =
                                Self::route(&invocation_id, next, invocations.clone()).await
                            {
                                error!("{}", error); // FIXME: Forward
                            }
                            continue;
                        }
                        unimplemented!()
                    }
                    Err(error) => {
                        for (id, sender) in Self::drain_current_invocations(invocations.clone()) {
                            let completion = Completion::<()>::error(
                                id.as_str(),
                                format!(
                                    "Terminating all client invocations due to JSON error {} while deserializing {}",
                                    error,
                                    next.to_string()
                                ),
                            );

                            let serialized = encoding.serialize(completion);
                            match serialized {
                                Ok(text) => {
                                    if let Err(error) = sender.send_async(text).await {
                                        error!("Error finalizing invocation : {}", error);
                                    }
                                }
                                Err(error) => {
                                    error!("error serializing completion error {}", error);

                                    let completion = Completion::<()>::error(
                                        id,
                                        "Terminating all invocations due to error deserializing message for unknown invocation",
                                    );

                                    let text = encoding.serialize(completion).expect("serialization of static object cannot go wrong if it went right once");

                                    if let Err(error) = sender.send_async(text).await {
                                        error!("Error finalizing invocation : {}", error);
                                    }
                                }
                            };
                        }
                    }
                }
            }
        };

        tokio::spawn(future);
    }

    async fn route(
        invocation_id: &String,
        text: ClientMessage,
        invocations: Arc<Mutex<HashMap<String, Sender<ClientMessage>>>>,
    ) -> Result<(), SignalRClientError> {
        let sender = {
            let invocations = invocations.lock().unwrap();
            invocations.get(invocation_id).and_then(|x| Some(x.clone()))
        };

        if let Some(sender) = sender {
            sender
                .send_async(text)
                .await
                .map_err(|x| -> ChannelSendError { x.into() })?;
        }

        Ok(())
    }

    fn drain_current_invocations(
        invocations: Arc<Mutex<HashMap<String, Sender<ClientMessage>>>>,
    ) -> Vec<(String, Sender<ClientMessage>)> {
        let mut invocations = invocations.lock().unwrap();
        invocations.drain().collect()
    }

    fn insert_invocation(&self, id: String, sender: Sender<ClientMessage>) {
        let mut invocations = self.invocations.lock().unwrap();
        (*invocations).insert(id, sender);
    }

    pub fn remove_invocation(&self, id: &String) {
        let mut invocations = self.invocations.lock().unwrap();
        (*invocations).remove(id);
    }
}

#[derive(Deserialize)]
struct MaybeInvocationId {
    #[serde(rename = "invocationId")]
    pub invocation_id: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct MessageTypeWrapper {
    #[serde(rename = "type")]
    pub message_type: MessageType,
}

pub struct ReceiveStream<T, I> {
    inner: Box<dyn Stream<Item = T> + Unpin>,
    invocations: Arc<Mutex<HashMap<String, Sender<I>>>>,
    invocation_id: String,
}

impl<T, I> Stream for ReceiveStream<T, I> {
    type Item = T;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.inner.poll_next_unpin(cx)
    }
}

impl<T, I> Drop for ReceiveStream<T, I> {
    fn drop(&mut self) {
        let mut invocations = self.invocations.lock().unwrap();
        invocations.remove(&self.invocation_id);
    }
}
