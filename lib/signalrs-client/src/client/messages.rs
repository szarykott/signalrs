use std::fmt::Display;

use serde::{de::DeserializeOwned, Serialize};

use crate::serialization;

use super::SignalRClientError;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(untagged)]
#[non_exhaustive]
pub enum ClientMessage {
    Json(String),
    Binary(Vec<u8>),
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
            ClientMessage::Json(value) => {
                let stripped = serialization::strip_record_separator(value.as_str());
                Ok(serde_json::from_str(stripped)?)
            }
            ClientMessage::Binary(_) => todo!(),
        }
    }

    pub fn get_encoding(&self) -> MessageEncoding {
        match self {
            ClientMessage::Json(_) => MessageEncoding::Json,
            ClientMessage::Binary(_) => todo!(),
        }
    }

    pub fn unwrap_text(&self) -> &str {
        match self {
            ClientMessage::Json(value) => &value,
            ClientMessage::Binary(_) => todo!(),
        }
    }

    pub fn split(self) -> Vec<ClientMessage> {
        match self {
            ClientMessage::Json(message) => message
                .split(serialization::RECORD_SEPARATOR)
                .filter(|item| !item.trim().is_empty())
                .map(|item| ClientMessage::Json(item.to_owned()))
                .collect(),
            ClientMessage::Binary(_) => unimplemented!(),
        }
    }
}

impl MessageEncoding {
    pub fn serialize(&self, message: impl Serialize) -> Result<ClientMessage, SignalRClientError> {
        match self {
            MessageEncoding::Json => {
                let json = serialization::to_json(&message)?;
                Ok(ClientMessage::Json(json))
            }
        }
    }
}

impl Display for ClientMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientMessage::Json(value) => write!(f, "{}", value),
            ClientMessage::Binary(_) => todo!(),
        }
    }
}
