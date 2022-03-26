use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::{collections::HashMap, convert::From};

#[derive(Debug, Serialize, Deserialize)]
/// Sent by the client to agree on the message format.
pub struct HandshakeRequest {
    protocol: String,
    version: u8,
}

impl HandshakeRequest {
    pub fn is_json(&self) -> bool {
        self.protocol == "json"
    }
}

#[derive(Debug, Serialize, Deserialize)]
/// Sent by the server as an acknowledgment of the previous `HandshakeRequest` message. Contains an error if the handshake failed.
pub struct HandshakeResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl HandshakeResponse {
    pub fn no_error() -> Self {
        HandshakeResponse { error: None }
    }

    pub fn error(reason: impl ToString) -> Self {
        HandshakeResponse {
            error: Some(reason.to_string()),
        }
    }
}

pub trait SignalRMessage {
    fn message_type(&self) -> MessageType;
}

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
/// Sent by the server when a connection is closed. Contains an error if the connection was closed because of an error.
pub struct Close {
    r#type: MessageType,
    error: Option<String>,
    allow_reconnect: Option<bool>,
}

impl Close {
    pub fn new(error: Option<String>, allow_reconnect: Option<bool>) -> Self {
        Close {
            r#type: MessageType::Close,
            error,
            allow_reconnect,
        }
    }
}

/// Indicates a request to invoke a particular method (the Target) with provided Arguments on the remote endpoint.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Invocation<A> {
    r#type: MessageType,
    headers: Option<HashMap<String, String>>,
    invocation_id: Option<String>,
    target: String,
    arguments: Option<A>,
    stream_ids: Option<Vec<String>>,
}

impl<A> Invocation<A> {
    pub fn new(invocation_id: Option<String>, target: String, arguments: Option<A>) -> Self {
        Invocation {
            r#type: MessageType::Invocation,
            headers: None,
            invocation_id,
            target,
            arguments,
            stream_ids: None,
        }
    }

    pub fn target(&self) -> &str {
        self.target.as_str()
    }

    pub fn id(&self) -> &Option<String> {
        &self.invocation_id
    }

    pub fn arguments(&mut self) -> Option<A> {
        self.arguments.take()
    }
}

#[derive(Debug, Serialize, Deserialize)]
/// Indicates a request to invoke a streaming method (the Target) with provided Arguments on the remote endpoint.
pub struct StreamInvocation<A> {
    r#type: MessageType,
    headers: Option<HashMap<String, String>>,
    pub invocation_id: String,
    target: String,
    pub arguments: Option<A>,
    stream_ids: Option<Vec<String>>,
}

impl<A> StreamInvocation<A> {
    pub fn new(invocation_id: String, target: String, arguments: Option<A>) -> Self {
        StreamInvocation {
            r#type: MessageType::StreamInvocation,
            headers: None,
            invocation_id,
            target,
            arguments,
            stream_ids: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
/// Indicates individual items of streamed response data from a previous `StreamInvocation` message.
pub struct StreamItem<I> {
    r#type: MessageType,
    headers: Option<HashMap<String, String>>,
    id: String,
    item: I,
}

impl<I> StreamItem<I> {
    pub fn new(id: String, item: I) -> Self {
        StreamItem {
            r#type: MessageType::StreamItem,
            headers: None,
            id,
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
    headers: Option<HashMap<String, String>>,
    invocation_id: String,
    result: Option<R>,
    error: Option<String>,
}

impl<R> Completion<R> {
    pub fn new(invocation_id: String, result: Option<R>, error: Option<String>) -> Self {
        Completion {
            r#type: MessageType::Completion,
            headers: None,
            invocation_id,
            result,
            error,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
/// Sent by the client to cancel a streaming invocation on the server.
pub struct CancelInvocation {
    r#type: MessageType,
    headers: Option<HashMap<String, String>>,
    id: String,
}

impl CancelInvocation {
    pub fn new(id: String) -> Self {
        CancelInvocation {
            r#type: MessageType::CancelInvocation,
            headers: None,
            id,
        }
    }
}

/// Message format used during SignalR exchange
#[derive(Debug, PartialEq, Eq, Hash)]
pub enum MessageFormat {
    /// A JSON format
    Json,
    /// A MessagePack format
    MessagePack,
}

impl MessageFormat {
    pub fn from_bytes<'de, T>(&self, bytes: &'de [u8]) -> T
    where
        T: Deserialize<'de>,
    {
        match self {
            MessageFormat::Json => serde_json::from_slice(bytes).unwrap(), // TODO: Fixme
            MessageFormat::MessagePack => rmp_serde::from_read_ref(bytes).unwrap(), // TODO: Fixm
        }
    }

    pub fn to_bytes<T>(&self, value: &T) -> Vec<u8>
    where
        T: Serialize,
    {
        match self {
            MessageFormat::Json => serde_json::to_vec(value).unwrap(), // TODO: Fixm
            MessageFormat::MessagePack => rmp_serde::to_vec(value).unwrap(), // TODO: Fixm
        }
    }
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

macro_rules! singalr_message {
    ($($t:ty),*) => {
        $(impl SignalRMessage for $t {
            fn message_type(&self) -> MessageType {
                self.r#type
            }
        })*
    };
}

singalr_message![Ping, CancelInvocation, Close];

impl<T> SignalRMessage for Invocation<T> {
    fn message_type(&self) -> MessageType {
        self.r#type
    }
}

impl<T> SignalRMessage for StreamInvocation<T> {
    fn message_type(&self) -> MessageType {
        self.r#type
    }
}

impl<T> SignalRMessage for StreamItem<T> {
    fn message_type(&self) -> MessageType {
        self.r#type
    }
}

impl<T> SignalRMessage for Completion<T> {
    fn message_type(&self) -> MessageType {
        self.r#type
    }
}
