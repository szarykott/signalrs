pub mod connection;
pub mod error;
pub mod extract;
pub mod hub;
pub mod invocation;
pub mod response;

use self::{
    connection::ConnectionState, error::SignalRError, hub::Hub, invocation::HubInvocation,
    response::ResponseSink,
};
use crate::{protocol::*, serialization};
use futures::SinkExt;
use log::*;
use serde_json::Value;

pub struct Server {
    hub: Hub,
}

impl Server {
    pub fn handshake(&self, input: &str) -> String {
        let input = serialization::strip_record_separator(input);

        let request = serde_json::from_str::<HandshakeRequest>(input);

        let response = match request {
            Ok(request) => {
                if request.is_json() {
                    HandshakeResponse::no_error()
                } else {
                    HandshakeResponse::error("Unsupported protocol")
                }
            }
            Err(e) => HandshakeResponse::error(e),
        };

        match serialization::to_json(&response) {
            Ok(value) => value,
            Err(e) => e.to_string(),
        }
    }

    pub async fn invoke_text(
        &self,
        text: String,
        connection_state: ConnectionState,
        output: ResponseSink,
    ) -> Result<(), SignalRError> {
        debug!("invoke_text invocation: {}", text);

        let result = self
            .invoke_text_internal(text, connection_state, output)
            .await;

        if let Err(ref e) = result {
            error!("invoke_text error: {}", e);
        } else {
            debug!("invoke_text success");
        }

        result
    }

    async fn invoke_text_internal(
        &self,
        text: String,
        connection_state: ConnectionState,
        mut output: ResponseSink,
    ) -> Result<(), SignalRError> {
        let text = serialization::strip_record_separator(&text);

        let RoutingData {
            target,
            message_type,
        } = serde_json::from_str(text)?;

        match message_type {
            MessageType::Invocation | MessageType::StreamInvocation => {
                let target = target.unwrap_or_else(|| {
                    error!("hub invoked without specifing target method");
                    "".to_owned()
                });

                if let Some(callable) = self.hub.methods.get(&target) {
                    let request = HubInvocation::text(text.to_owned(), connection_state, output)?;
                    callable.call(request).await?;
                } else {
                    error!("method '{target}' not found")
                }
            }
            MessageType::CancelInvocation => {
                let message: CancelInvocation = serde_json::from_str(text)?;

                connection_state
                    .inflight_invocations
                    .cancel(&message.invocation_id);
            }
            MessageType::StreamItem => {
                let message: StreamItem<Value> = serde_json::from_str(text)?;

                let upload_sink = connection_state
                    .upload_sinks
                    .get_sink(&message.invocation_id);

                if let Some(mut upload_sink) = upload_sink {
                    upload_sink.send(message.item).await?;
                } else {
                    warn!("received upload stream item without matching invocation")
                }
            }
            MessageType::Completion => {
                let message: Id = serde_json::from_str(text)?;

                connection_state.upload_sinks.remove(&message.invocation_id);
            }
            MessageType::Ping => {
                let ping = Ping::new();
                output.send(serialization::to_json(&ping)?).await?;
            }
            MessageType::Close => {
                warn!("received close message, supposedly only sent by the server")
            }
            MessageType::Other => {
                error!("received message of unknown type");
            }
        };

        Ok(())
    }
}

impl From<Hub> for Server {
    fn from(hub: Hub) -> Self {
        Server { hub }
    }
}
