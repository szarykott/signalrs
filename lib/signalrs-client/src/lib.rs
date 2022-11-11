#![deny(unsafe_code)]

mod builder;
mod client;
mod error;
mod hub;
mod messages;
pub mod negotiate;
mod parts;
pub mod protocol;
mod send_builder;
mod serialization;
mod stream_ext;
mod websocket;

pub use self::{
    builder::ClientBuilder,
    client::SignalRClient,
    error::{ChannelSendError, SignalRClientError},
    hub::Hub,
    parts::{IntoInvocationPart, InvocationPart, InvocationStream},
    send_builder::SendBuilder,
};
