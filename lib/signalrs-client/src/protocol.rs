use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::{collections::HashMap, convert::From, fmt::Display};

use crate::messages;

pub const WEB_SOCKET_TRANSPORT: &str = "WebSockets";
pub const TEXT_TRANSPORT_FORMAT: &str = "Text";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NegotiateResponseV0 {
    pub connection_id: String,
    pub negotiate_version: u8,
    pub available_transports: Vec<TransportSpec>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportSpec {
    pub transport: String,
    pub transfer_formats: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Sent by the client to agree on the message format.
pub struct HandshakeRequest {
    protocol: String,
    version: u8,
}

impl HandshakeRequest {
    pub fn new(protocol: impl ToString) -> Self {
        HandshakeRequest {
            protocol: protocol.to_string(),
            version: 1,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Sent by the server as an acknowledgment of the previous `HandshakeRequest` message. Contains an error if the handshake failed.
pub struct HandshakeResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}

impl HandshakeResponse {
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }

    pub fn unwrap_error(self) -> String {
        self.error.unwrap()
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Sent by either party to check if the connection is active.
pub struct Ping {
    r#type: MessageType,
}

impl Ping {
    pub fn new() -> Self {
        Ping {
            r#type: MessageType::Ping,
        }
    }
}

impl Default for Ping {
    fn default() -> Self {
        Ping::new()
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Sent by the server when a connection is closed. Contains an error if the connection was closed because of an error.
pub struct Close {
    r#type: MessageType,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    allow_reconnect: Option<bool>,
}

/// Indicates a request to invoke a particular method (the Target) with provided Arguments on the remote endpoint.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Invocation<A> {
    r#type: MessageType,
    #[serde(skip_serializing_if = "Option::is_none")]
    headers: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    invocation_id: Option<String>,
    target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    arguments: Option<A>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_ids: Option<Vec<String>>,
}

impl<A> Invocation<A> {
    pub fn non_blocking(target: impl Into<String>, arguments: Option<A>) -> Self {
        Invocation {
            r#type: MessageType::Invocation,
            headers: None,
            invocation_id: None,
            target: target.into(),
            arguments,
            stream_ids: None,
        }
    }

    pub fn with_invocation_id(&mut self, invocation_id: impl ToString) -> &mut Self {
        self.invocation_id = Some(invocation_id.to_string());
        self
    }

    pub fn with_streams(&mut self, stream_ids: Vec<String>) -> &mut Self {
        if !stream_ids.is_empty() {
            self.stream_ids = Some(stream_ids);
        }
        self
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Indicates a request to invoke a streaming method (the Target) with provided Arguments on the remote endpoint.
pub struct StreamInvocation<A> {
    r#type: MessageType,
    #[serde(skip_serializing_if = "Option::is_none")]
    headers: Option<HashMap<String, String>>,
    invocation_id: String,
    target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    arguments: Option<A>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream_ids: Option<Vec<String>>,
}

impl<A> StreamInvocation<A> {
    pub fn new(
        invocation_id: impl Into<String>,
        target: impl Into<String>,
        arguments: Option<A>,
    ) -> Self {
        StreamInvocation {
            r#type: MessageType::StreamInvocation,
            headers: None,
            invocation_id: invocation_id.into(),
            target: target.into(),
            arguments,
            stream_ids: None,
        }
    }

    pub fn with_streams(&mut self, stream_ids: Vec<String>) -> &mut Self {
        if !stream_ids.is_empty() {
            self.stream_ids = Some(stream_ids);
        }
        self
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
/// Indicates individual items of streamed response data from a previous `StreamInvocation` message.
pub struct StreamItem<I> {
    r#type: MessageType,
    #[serde(skip_serializing_if = "Option::is_none")]
    headers: Option<HashMap<String, String>>,
    pub(crate) invocation_id: String,
    pub(crate) item: I,
}

impl<I> StreamItem<I> {
    pub fn new(invocation_id: impl Into<String>, item: I) -> Self {
        StreamItem {
            r#type: MessageType::StreamItem,
            headers: None,
            invocation_id: invocation_id.into(),
            item,
        }
    }
}

/// Indicates a previous Invocation or StreamInvocation has completed.
/// Contains an error if the invocation concluded with an error or the result of a non-streaming method invocation.
/// The result will be absent for void methods.
/// In case of streaming invocations no further StreamItem messages will be received.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Completion<R> {
    r#type: MessageType,
    #[serde(skip_serializing_if = "Option::is_none")]
    headers: Option<HashMap<String, String>>,
    invocation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<R>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl<R> Completion<R> {
    pub fn ok(invocation_id: impl Into<String>) -> Self {
        Self::new(invocation_id, None, None)
    }

    pub fn error(invocation_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self::new(invocation_id, None, Some(error.into()))
    }

    pub fn new(invocation_id: impl Into<String>, result: Option<R>, error: Option<String>) -> Self {
        Completion {
            r#type: MessageType::Completion,
            headers: None,
            invocation_id: invocation_id.into(),
            result,
            error,
        }
    }

    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }

    pub fn is_result(&self) -> bool {
        self.result.is_some()
    }

    pub fn unwrap_error(self) -> String {
        self.error.unwrap()
    }

    pub fn unwrap_result(self) -> R {
        self.result.unwrap()
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Sent by the client to cancel a streaming invocation on the server.
pub struct CancelInvocation {
    r#type: MessageType,
    #[serde(skip_serializing_if = "Option::is_none")]
    headers: Option<HashMap<String, String>>,
    pub invocation_id: String,
}

#[derive(Debug, Serialize_repr, Deserialize_repr, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    Invocation = 1,
    StreamItem = 2,
    Completion = 3,
    StreamInvocation = 4,
    CancelInvocation = 5,
    Ping = 6,
    Close = 7,
    Other = 8,
}

impl Display for MessageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageType::Invocation => write!(f, "{}", stringify!(MessageType::Invocation)),
            MessageType::StreamItem => write!(f, "{}", stringify!(MessageType::StreamItem)),
            MessageType::Completion => write!(f, "{}", stringify!(MessageType::Completion)),
            MessageType::StreamInvocation => {
                write!(f, "{}", stringify!(MessageType::StreamInvocation))
            }
            MessageType::CancelInvocation => {
                write!(f, "{}", stringify!(MessageType::CancelInvocation))
            }
            MessageType::Ping => write!(f, "{}", stringify!(MessageType::Ping)),
            MessageType::Close => write!(f, "{}", stringify!(MessageType::Close)),
            MessageType::Other => write!(f, "{}", stringify!(MessageType::Other)),
        }
    }
}

impl From<u8> for MessageType {
    fn from(i: u8) -> Self {
        match i {
            1 => MessageType::Invocation,
            2 => MessageType::StreamItem,
            3 => MessageType::Completion,
            4 => MessageType::StreamInvocation,
            5 => MessageType::CancelInvocation,
            6 => MessageType::Ping,
            7 => MessageType::Close,
            _ => MessageType::Other,
        }
    }
}

// TODO: Try to unify
#[derive(Deserialize, Debug, Clone)]
pub struct OptionalId {
    #[serde(rename = "invocationId")]
    pub invocation_id: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Id {
    #[serde(rename = "invocationId")]
    pub invocation_id: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RoutingData {
    pub target: Option<String>,
    #[serde(rename = "type")]
    pub message_type: MessageType,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Arguments<T> {
    pub arguments: Option<T>,
}

#[derive(Deserialize, Debug)]
pub struct ClientStreams {
    #[serde(rename = "streamIds")]
    pub stream_ids: Option<Vec<String>>,
}

impl TryInto<String> for HandshakeRequest {
    type Error = serde_json::Error;

    fn try_into(self) -> Result<String, Self::Error> {
        messages::to_json(&self)
    }
}

impl TryFrom<String> for HandshakeResponse {
    type Error = serde_json::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let s = messages::strip_record_separator(&value);
        serde_json::from_str(s)
    }
}
