mod arguments;
mod functions;
pub mod invocation;

use self::{
    functions::{Handler, HandlerWrapper, HubMethod},
    invocation::HubInvocation,
};
use crate::protocol::MessageType;
use serde::Deserialize;
use std::collections::HashMap;
use tracing::*;

use super::{messages::ClientMessage, SignalRClientError};

#[derive(Default)]
pub struct Hub {
    methods: HashMap<String, Box<dyn HubMethod + Send + Sync + 'static>>,
}

impl Hub {
    pub fn method<M, Args>(mut self, name: impl ToString, method: M) -> Self
    where
        M: Handler<Args> + Send + Sync + Clone + 'static,
        Args: Send + Sync + 'static,
    {
        if let Some(_) = self
            .methods
            .insert(name.to_string(), Box::new(HandlerWrapper::<M, Args>::from(method)))
        {
            warn!("overwritten method {}", name.to_string())
        }

        self
    }

    pub fn call(&self, message: ClientMessage) -> Result<(), SignalRClientError> {
        let RoutingData {
            message_type,
            target,
        } = message.deserialize()?;

        match message_type {
            MessageType::Invocation => self.invocation(target, message),
            x => self.unsupported(x),
        }
    }

    fn invocation(
        &self,
        target: Option<String>,
        message: ClientMessage,
    ) -> Result<(), SignalRClientError> {
        let target = target.ok_or_else(|| SignalRClientError::ProtocolError {
            message: "Target of invocation missing in request".into(),
        })?;

        let method = self
            .methods
            .get(&target)
            .ok_or_else(|| SignalRClientError::HubError(format!("target {} not found", target)))?;

        method.call(HubInvocation::new(message))
    }

    fn unsupported(&self, message_type: MessageType) -> Result<(), SignalRClientError> {
        Err(SignalRClientError::ProtocolError {
            message: format!("{message_type} not supported by client-side hub"),
        })
    }
}

#[derive(Deserialize)]
struct RoutingData {
    #[serde(rename = "type")]
    message_type: MessageType,
    target: Option<String>,
}
