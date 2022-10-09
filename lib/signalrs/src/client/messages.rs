use std::fmt::Display;

use serde::{de::DeserializeOwned, Serialize};

use super::SignalRClientError;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(untagged)]
#[non_exhaustive]
pub enum ClientMessage {
    Json(String),
}

#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum MessageEncoding {
    Json,
}

impl ClientMessage {
    pub fn deserialize<T>(&self) -> Result<T, SignalRClientError>
    where
        T: DeserializeOwned,
    {
        match self {
            ClientMessage::Json(value) => Ok(serde_json::from_str(&value)?),
        }
    }

    pub fn unwrap_text(self) -> String {
        match self {
            ClientMessage::Json(value) => value,
        }
    }
}

impl MessageEncoding {
    pub fn serialize(&self, message: impl Serialize) -> Result<ClientMessage, SignalRClientError> {
        match self {
            MessageEncoding::Json => Ok(ClientMessage::Json(serde_json::to_string(&message)?)),
        }
    }
}

impl Display for ClientMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientMessage::Json(value) => write!(f, "{}", value),
        }
    }
}
