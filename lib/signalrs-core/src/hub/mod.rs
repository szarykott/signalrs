use std::{collections::HashMap, pin::Pin, sync::Arc};

use flume::r#async::SendSink;
use futures::{Future, SinkExt};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::{
    error::SignalRError,
    handler::Callable,
    protocol::*,
    request::{HubRequest, StreamItemPayload},
    response::ResponseSink,
};

pub mod builder;

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
    inflight_invocations: Arc<Mutex<HashMap<String, tokio::task::JoinHandle<()>>>>,
    client_streams_mapping: Arc<Mutex<HashMap<String, ClientSink>>>,
}

#[allow(missing_debug_implementations)]
pub struct ClientSink {
    pub sink: SendSink<'static, StreamItemPayload>,
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
                    let request = HubRequest::text(
                        text,
                        Arc::clone(&self.inflight_invocations),
                        Arc::clone(&self.client_streams_mapping),
                    );

                    callable.call(request, output).await?;
                }
            }
            MessageType::CancelInvocation => {
                let message: CancelInvocation = serde_json::from_str(&text)?;

                let mut guard = self.inflight_invocations.lock().await;
                match (*guard).remove(&message.invocation_id) {
                    Some(handle) => handle.abort(),
                    None => { /* all good */ }
                };
            }
            MessageType::StreamItem => {
                let message: StreamItem<Value> = serde_json::from_str(&text)?;

                let mut guard = self.client_streams_mapping.lock().await;
                let cs = (*guard).get_mut(&message.invocation_id); // invocation id == stream id from invocation

                if let Some(cs) = cs {
                    dbg!(message.item.clone());
                    cs.sink.send(StreamItemPayload::Text(message.item)).await?;
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
