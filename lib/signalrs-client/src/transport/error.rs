use thiserror::Error;

use crate::error::ClientError;

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
        source: tokio_tungstenite::tungstenite::Error,
    },

    #[error("bad message receive")]
    BadReceive,

    #[error("client error")]
    ClientError {
        #[from]
        source: ClientError,
    },
}
