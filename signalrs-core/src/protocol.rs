use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use signalrs_error::SignalRError;
use std::{
    collections::HashMap,
    convert::{From, TryFrom, TryInto},
};

#[derive(Debug, Serialize, Deserialize)]
/// Sent by the client to agree on the message format.
pub struct HandshakeRequest {
    protocol: String,
    version: u8,
}

#[derive(Debug, Serialize, Deserialize)]
/// Sent by the server as an acknowledgment of the previous `HandshakeRequest` message. Contains an error if the handshake failed.
pub struct HandshakeResponse {
    error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
/// Sent by either party to check if the connection is active.
pub struct Ping {
    r#type: u8,
}

impl Ping {
    pub fn new() -> Self {
        Ping { r#type: PING }
    }
}

#[derive(Debug, Serialize, Deserialize)]
/// Sent by the server when a connection is closed. Contains an error if the connection was closed because of an error.
pub struct Close {
    r#type: u8,
    error: Option<String>,
    allow_reconnect: Option<bool>,
}

impl Close {
    pub fn new(error: Option<String>, allow_reconnect: Option<bool>) -> Self {
        Close {
            r#type: CLOSE,
            error,
            allow_reconnect,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
/// Indicates a request to invoke a particular method (the Target) with provided Arguments on the remote endpoint.
pub struct Invocation {
    r#type: u8,
    headers: Option<HashMap<String, String>>,
    invocation_id: Option<String>,
    target: String,
    arguments: Object,
    stream_ids: Option<Vec<String>>,
}

impl Invocation {
    pub fn arguments(&self) -> &Object {
        &self.arguments
    }

    pub fn target(&self) -> &str {
        self.target.as_str()
    }
}

#[derive(Debug, Serialize, Deserialize)]
/// Indicates a request to invoke a streaming method (the Target) with provided Arguments on the remote endpoint.
pub struct StreamInvocation {
    r#type: u8,
    headers: Option<HashMap<String, String>>,
    invocation_id: String,
    target: String,
    arguments: Object,
}

#[derive(Debug, Serialize, Deserialize)]
/// Indicates individual items of streamed response data from a previous `StreamInvocation` message.
pub struct StreamItem {
    r#type: u8,
    headers: Option<HashMap<String, String>>,
    invocation_id: String,
    item: Object,
}

impl StreamItem {
    pub fn new(invocation_id: String, item: Object) -> Self {
        StreamItem {
            r#type: STREAM_ITEM,
            headers: None,
            invocation_id,
            item,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
/// Indicates a previous Invocation or StreamInvocation has completed.
/// Contains an error if the invocation concluded with an error or the result of a non-streaming method invocation.
/// The result will be absent for void methods.
/// In case of streaming invocations no further StreamItem messages will be received.
pub struct Completion {
    r#type: u8,
    headers: Option<HashMap<String, String>>,
    invocation_id: String,
    result: Option<Object>,
    error: Option<String>,
}

impl Completion {
    pub fn new(invocation_id: String, result: Option<Object>, error: Option<String>) -> Self {
        Completion {
            r#type: COMPLETION,
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
    r#type: u8,
    headers: Option<HashMap<String, String>>,
    invocation_id: String,
}

impl CancelInvocation {
    pub fn new(invocation_id: String) -> Self {
        CancelInvocation {
            r#type: CANCEL_INVOCATION,
            headers: None,
            invocation_id,
        }
    }
}

/// SignalR message
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Message {
    HandshakeRequest(HandshakeRequest),
    HandshakeResponse(HandshakeResponse),
    Close(Close),
    Invocation(Invocation),
    StreamInvocation(StreamInvocation),
    StreamItem(StreamItem),
    Completion(Completion),
    CancelInvocation(CancelInvocation),
    Ping(Ping),
}

/// Message format used during SignalR exchange
#[derive(Debug, PartialEq, Eq, Hash)]
pub enum MessageFormat {
    /// A JSON format
    Json,
    /// A MessagePack format
    MessagePack,
}

pub const INVOCATION: u8 = 1;
pub const STREAM_ITEM: u8 = 2;
pub const COMPLETION: u8 = 3;
pub const STREAM_INVOCATION: u8 = 4;
pub const CANCEL_INVOCATION: u8 = 5;
pub const PING: u8 = 6;
pub const CLOSE: u8 = 7;

#[derive(Debug, Serialize_repr, Deserialize_repr)]
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

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Object {
    Nil,
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Array(Vec<Object>),
    Obj(HashMap<String, Object>),
}

// macro_rules! try_from_for {
//     ($($t:ty),*) => {
//         $(impl TryFrom<&Object> for $t {
//             type Error = SignalRError;

//             fn try_from(value: &Object) -> Result<Self, Self::Error> {
//                 match value {
//                     Object::Int(v) => TryInto::try_into(*v as $t),
//                     _ => Err(SignalRError::ProtocolViolation("String".to_string()))
//                 }
//             }
//         })*
//     };
// }

// try_from_for!(i8, i16, i32, i64, isize, f32, f64, bool, String);

impl From<StreamItem> for Message {
    fn from(si: StreamItem) -> Self {
        Message::StreamItem(si)
    }
}

impl From<Ping> for Message {
    fn from(m: Ping) -> Self {
        Message::Ping(m)
    }
}

impl From<Completion> for Message {
    fn from(m: Completion) -> Self {
        Message::Completion(m)
    }
}

impl From<Close> for Message {
    fn from(m: Close) -> Self {
        Message::Close(m)
    }
}
