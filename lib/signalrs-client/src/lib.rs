#![deny(unsafe_code)]

mod builder;
mod caller;
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
    hub::Hub,
};
