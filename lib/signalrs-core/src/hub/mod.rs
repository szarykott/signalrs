use std::{any::Any, collections::HashMap, pin::Pin, sync::Arc};

use futures::Future;
use tokio::sync::Mutex;

use crate::{
    error::SignalRError,
    handler::Callable,
    protocol::{HandshakeRequest, HandshakeResponse, MessageType, Target, Type},
    request::HubRequest,
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
    client_streams_mapping: Arc<Mutex<HashMap<String, ClientStream>>>,
}

pub struct ClientStream {
    to_function: String,
    sink: Box<dyn Any + Send>,
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
        output: ResponseSink,
    ) -> Result<(), SignalRError> {
        let text = text.trim_end_matches(WEIRD_ENDING).to_owned();

        match serde_json::from_str::<Type>(&text)?.message_type {
            MessageType::Invocation | MessageType::StreamInvocation => {
                let target: Target = serde_json::from_str(&text)?;

                if let Some(callable) = self.methods.get(&target.target) {
                    let request = HubRequest::text(text, Arc::clone(&self.inflight_invocations));

                    callable.call(request, output, false).await?;
                }
            }
            _ => panic!(),
        };

        Ok(())
    }
}
