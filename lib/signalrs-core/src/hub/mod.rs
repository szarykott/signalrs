pub mod builder;
pub mod client_sink;

use std::{collections::HashMap, pin::Pin, sync::Arc};

use futures::{Future, SinkExt};
use serde_json::Value;

use crate::{
    connection::{ConnectionState, StreamItemPayload},
    error::SignalRError,
    handler::callable::Callable,
    invocation::HubInvocation,
    protocol::*,
    response::ResponseSink,
    serialization,
};

#[allow(missing_debug_implementations)]
pub struct Hub {
    methods: HashMap<
        String,
        Arc<
            dyn Callable<Future = Pin<Box<dyn Future<Output = Result<(), SignalRError>> + Send>>>
                + Send
                + Sync,
        >,
    >,
}

impl Hub {
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
        mut output: ResponseSink,
    ) -> Result<(), SignalRError> {
        let text = serialization::strip_record_separator(&text).to_owned();

        // TODO: implement mechanism to only enable one request from client / connection
        // to be processed at the time
        // then some things could be lockless down the stream

        let RoutingData {
            target,
            message_type,
        } = serde_json::from_str(&text)?;

        match message_type {
            MessageType::Invocation | MessageType::StreamInvocation => {
                if let Some(callable) = self.methods.get(&target.unwrap_or_default()) {
                    let request = HubInvocation::text(text, connection_state.clone());

                    callable.call(request, output).await?;
                }
            }
            MessageType::CancelInvocation => {
                let message: CancelInvocation = serde_json::from_str(&text)?;

                connection_state
                    .inflight_invocations
                    .cancel(&message.invocation_id);
            }
            MessageType::StreamItem => {
                let message: StreamItem<Value> = serde_json::from_str(&text)?;

                let cs = connection_state
                    .client_streams_mapping
                    .get_sink(&message.invocation_id);

                if let Some(mut cs) = cs {
                    cs.send(StreamItemPayload::Text(message.item)).await?;
                }
            }
            MessageType::Completion => {
                let message: Id = serde_json::from_str(&text)?;

                connection_state
                    .client_streams_mapping
                    .remove(&message.invocation_id);
            }
            MessageType::Ping => {
                let ping = Ping::new();
                output.send(serialization::to_json(&ping)?).await?;
            }
            MessageType::Close => todo!(),
            MessageType::Other => {
                /* panik or kalm? */
                todo!()
            }
        };

        Ok(())
    }
}
