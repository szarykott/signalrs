#![deny(unsafe_code)]

mod builder;
mod caller;
mod error;
mod hub;
mod messages;
pub mod protocol;
mod transport;

pub use self::{
    builder::ClientBuilder,
    caller::{
        parts::{IntoInvocationPart, InvocationPart, InvocationStream},
        send_builder::SendBuilder,
        SignalRClient,
    },
    error::SignalRClientError,
    hub::Hub,
};
