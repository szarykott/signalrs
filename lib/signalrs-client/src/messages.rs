use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Display;
use thiserror::Error;

pub const RECORD_SEPARATOR: &str = "\u{001E}";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(untagged)]
#[non_exhaustive]
pub(crate) enum ClientMessage {
    Json(String),
    Binary(Vec<u8>),
}

#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub(crate) enum MessageEncoding {
    Json,
}

#[derive(Debug, Error)]
pub enum SerializationError {
    #[error("JSON error occured: {source}")]
    Json {
        #[from]
        source: serde_json::Error,
    },
}

impl ClientMessage {
    pub fn deserialize<T>(&self) -> Result<T, SerializationError>
    where
        T: DeserializeOwned,
    {
        match self {
            ClientMessage::Json(value) => {
                let stripped = strip_record_separator(value.as_str());
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
                .split(RECORD_SEPARATOR)
                .filter(|item| !item.trim().is_empty())
                .map(|item| ClientMessage::Json(item.to_owned()))
                .collect(),
            ClientMessage::Binary(_) => unimplemented!(),
        }
    }
}

impl MessageEncoding {
    pub fn serialize(&self, message: impl Serialize) -> Result<ClientMessage, SerializationError> {
        match self {
            MessageEncoding::Json => {
                let json = to_json(&message)?;
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

pub fn to_json<T>(value: &T) -> Result<String, serde_json::Error>
where
    T: ?Sized + Serialize,
{
    let serialized = serde_json::to_string(value)?;
    Ok(serialized + RECORD_SEPARATOR)
}

pub fn strip_record_separator(input: &str) -> &str {
    input.trim_end_matches(RECORD_SEPARATOR)
}
