use flume::{Receiver, Sender};
use futures::{Stream, StreamExt};
use log::*;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use super::{ChannelSendError, SignalRClientError};
use crate::protocol::Completion;

pub struct SignalRClientReceiver<S, I> {
    pub(super) incoming_messages: Option<S>,
    pub(super) invocations: Arc<Mutex<HashMap<String, Sender<I>>>>,
}

impl<S> SignalRClientReceiver<S, String>
where
    S: Stream<Item = String> + Send + Unpin + 'static,
{
    pub async fn receive_once<T: DeserializeOwned>(
        &self,
        invocation_id: impl Into<String>,
    ) -> Result<Result<T, String>, SignalRClientError> {
        let (tx, rx) = flume::bounded::<String>(1);
        self.insert_invocation(invocation_id.into(), tx);

        let text = rx.recv_async().await?;
        let completion: Completion<T> = serde_json::from_str(&text)?;

        if completion.is_ok() {
            return Ok(Ok(completion.get_result_owned().unwrap()));
        }

        if completion.is_error() {
            return Ok(Err(completion.get_error_owned().unwrap()));
        }

        Err(SignalRClientError::ProtocolError {
            message: "Callee completed invocation without success or error indicator".into(),
        })
    }

    pub fn receive_stream<T>(
        &self,
        invocation_id: impl Into<String>,
    ) -> Result<ReceiveStream<Result<T, SignalRClientError>>, SignalRClientError> {
        todo!()
    }

    pub(super) fn start_receiver_loop(&mut self) {
        let mut incoming = self
            .incoming_messages
            .take()
            .expect("Improper receiver initialization");

        let invocations = self.invocations.clone();

        let future = async move {
            while let Some(next) = incoming.next().await {
                match serde_json::from_str::<MaybeInvocationId>(&next) {
                    Ok(maybe_id) => {
                        if let Some(invocation_id) = maybe_id.invocation_id {
                            if let Err(error) =
                                Self::route_text(&invocation_id, next, invocations.clone()).await
                            {
                                unimplemented!()
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

                            match serde_json::to_string(&completion) {
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

                                    let text = serde_json::to_string(&completion).expect("serialization of static object cannot go wrong if it went right once");

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

    async fn route_text(
        invocation_id: &String,
        text: String,
        invocations: Arc<Mutex<HashMap<String, Sender<String>>>>,
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
        invocations: Arc<Mutex<HashMap<String, Sender<String>>>>,
    ) -> Vec<(String, Sender<String>)> {
        let mut invocations = invocations.lock().unwrap();
        invocations.drain().collect()
    }

    fn insert_invocation(&self, id: String, sender: Sender<String>) {
        let mut invocations = self.invocations.lock().unwrap();
        (*invocations).insert(id, sender);
    }
}

pub struct ReceiveStream<T>(Box<dyn Stream<Item = T> + Unpin>);

#[derive(Deserialize)]
struct MaybeInvocationId {
    #[serde(rename = "invocationId")]
    pub invocation_id: Option<String>,
}
