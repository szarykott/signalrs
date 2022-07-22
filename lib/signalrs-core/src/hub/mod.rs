pub mod builder;
pub mod client_sink;
pub mod client_stream_map;
pub mod inflight_invocations;

use std::{collections::HashMap, pin::Pin, sync::Arc};

use futures::{Future, SinkExt};
use serde_json::Value;

use crate::{
    error::SignalRError,
    handler::callable::Callable,
    protocol::*,
    request::{HubInvocation, StreamItemPayload},
    response::ResponseSink,
};

use self::{client_stream_map::ClientStreamMap, inflight_invocations::InflightInvocations};

const WEIRD_ENDING: &str = "\u{001E}";

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
    inflight_invocations: InflightInvocations,
    client_streams_mapping: ClientStreamMap,
}

impl Hub {
    pub fn handshake(&self, input: &str) -> String {
        let input = input.trim_end_matches(WEIRD_ENDING);

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

        match serde_json::to_string(&response) {
            Ok(value) => format!("{}{}", value, WEIRD_ENDING),
            Err(e) => e.to_string(),
        }
    }

    pub async fn invoke_text(
        &self,
        text: String,
        mut output: ResponseSink,
    ) -> Result<(), SignalRError> {
        let text = text.trim_end_matches(WEIRD_ENDING).to_owned();

        let RoutingData {
            target,
            message_type,
        } = serde_json::from_str(&text)?;

        match message_type {
            MessageType::Invocation | MessageType::StreamInvocation => {
                if let Some(callable) = self.methods.get(&target.unwrap_or_default()) {
                    let request = HubInvocation::text(
                        text,
                        self.inflight_invocations.clone(),
                        self.client_streams_mapping.clone(),
                    );

                    callable.call(request, output).await?;
                }
            }
            MessageType::CancelInvocation => {
                let message: CancelInvocation = serde_json::from_str(&text)?;

                self.inflight_invocations
                    .cancel(&message.invocation_id)
                    .await;
            }
            MessageType::StreamItem => {
                let message: StreamItem<Value> = serde_json::from_str(&text)?;

                let mut guard = self.client_streams_mapping.lock().await;
                let cs = (*guard).get_mut(&message.invocation_id); // invocation id == stream id from invocation

                if let Some(cs) = cs {
                    cs.send(StreamItemPayload::Text(message.item)).await?;
                }
            }
            MessageType::Completion => {
                let message: Id = serde_json::from_str(&text)?;

                let mut guard = self.client_streams_mapping.lock().await;
                let cs = (*guard).remove(&message.invocation_id);

                if let Some(cs) = cs {
                    drop(cs); // should terminate sender
                }
            }
            MessageType::Ping => {
                let ping = Ping::new();
                let s = serde_json::to_string(&ping)?;
                output.send(s + WEIRD_ENDING).await?;
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
