//! Client-side hub

pub mod arguments;
pub mod error;
mod functions;
pub(crate) mod invocation;

use self::{
    error::{HubError, MalformedRequest},
    functions::{Handler, HandlerWrapper, HubMethod},
    invocation::HubInvocation,
};
use super::messages::ClientMessage;
use crate::protocol::MessageType;
use serde::Deserialize;
use std::collections::HashMap;
use tracing::*;

/// Client-side hub
///
/// [`Hub`] can be called by the server. It currently supports only value-like arguments - ones that can be deserialized from a single message.
/// There are also stream-like arguments - ones that server can stream to client asynchronously, but they are not supported yet.
/// ```rust, no_run
/// use signalrs_client::SignalRClient;
/// use signalrs_client::hub::Hub;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let hub = Hub::default().method("Send", print);    
///
///     let client = SignalRClient::builder("localhost")
///         .use_port(8080)
///         .use_hub("echo")
///         .with_client_hub(hub)
///         .build()
///         .await?;
/// # Ok(())
/// }
///
/// // Hub methods need to be async
/// async fn print(message: String) {
///     println!("{message}");
/// }
/// ```
#[derive(Default)]
pub struct Hub {
    methods: HashMap<String, Box<dyn HubMethod + Send + Sync + 'static>>,
}

impl Hub {
    /// Embeds a new method in a hub under `name`.
    ///
    /// Any server calls to a method of `name` will be routed to function pointed to by `method`.
    /// Only functions returning `()` and `async` are allowed.
    /// Up to 13 arguments are supported. They will be extracted from server call.
    /// In case extraction fails, error will be logged.
    ///
    /// All primitive arguments should be usable out of the box. In case custom one needs to be used see [`HubArgument`](signalrs_derive::HubArgument).
    /// Value-like hub arguments need to implement [`Deserialize`](serde::Deserialize) as well.
    ///
    /// # Example
    /// ```rust,no_run
    /// use serde::Deserialize;
    /// use signalrs_derive::HubArgument;
    /// # use signalrs_client::hub::Hub;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// #     let hub = Hub::default().method("Send", print);    
    /// #     Ok(())
    /// # }
    /// # async fn print(_message: Data) {
    /// #    // do nothing
    /// # }
    ///
    /// #[derive(Deserialize, HubArgument)]
    /// struct Data {
    ///     f1: i32,
    ///     f2: String
    /// }
    /// ```
    pub fn method<M, Args>(mut self, name: impl ToString, method: M) -> Self
    where
        M: Handler<Args> + Send + Sync + Clone + 'static,
        Args: Send + Sync + 'static,
    {
        if self
            .methods
            .insert(name.to_string(), Box::new(HandlerWrapper::<M, Args>::from(method)))
            .is_some()
        {
            warn!("overwritten method {}", name.to_string())
        }

        self
    }

    pub(crate) fn call(&self, message: ClientMessage) -> Result<(), HubError> {
        let RoutingData {
            message_type,
            target,
        } = message
            .deserialize()
            .map_err(|error| -> MalformedRequest { error.into() })?;

        match message_type {
            MessageType::Invocation => self.invocation(target, message),
            x => self.unsupported(x),
        }
    }

    fn invocation(&self, target: Option<String>, message: ClientMessage) -> Result<(), HubError> {
        let target = target.ok_or_else(|| HubError::Unprocessable {
            message: "Target of invocation missing in request".into(),
        })?;

        let method = self
            .methods
            .get(&target)
            .ok_or_else(|| HubError::Unprocessable {
                message: format!("target {} not found", target),
            })?;

        method.call(HubInvocation::new(message)?)
    }

    fn unsupported(&self, message_type: MessageType) -> Result<(), HubError> {
        Err(HubError::Unsupported {
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
