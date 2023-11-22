use thiserror::Error;

use crate::error::ClientError;

#[cfg(feature = "tokio-rt")]
pub type WebSocketError = tokio_tungstenite::tungstenite::Error;
#[cfg(feature = "async-std-rt")]
pub type WebSocketError = async_tungstenite::tungstenite::Error;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("serialization error: {source}")]
    Serialization {
        #[from]
        source: serde_json::Error,
    },

    #[error("WebSockets error")]
    Websocket {
        #[from]
        source: WebSocketError,
    },

    #[error("bad message receive")]
    BadReceive,

    #[error("client error")]
    ClientError {
        #[from]
        source: ClientError,
    },
}
