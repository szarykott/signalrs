use thiserror::Error;

use super::{hub::invocation::ExtractionError, messages::ClientMessage};

#[derive(Error, Debug)]
pub enum SignalRClientError {
    #[error("Json error: {source}")]
    JsonError {
        #[from]
        source: serde_json::Error,
    },
    #[error("Error while receiving message")]
    ReceiveError {
        #[from]
        source: flume::RecvError,
    },
    #[error("Error while sending message")]
    SendError {
        #[from]
        source: ChannelSendError,
    },
    #[error("Protocol error occured")]
    ProtocolError { message: String },
    #[error("Invocation finished with error")]
    InvocationError { message: String },
    #[error("Error during extraction from invocation")]
    ExtractionError {
        #[from]
        source: ExtractionError,
    },
    #[error("Hub error occured")]
    HubError(String),
    #[error("WebSocket error")]
    WebSocketError {
        #[from]
        source: tokio_tungstenite::tungstenite::Error,
    },
    #[error("tokio oneshot error")]
    TokioOneshotError {
        #[from]
        source: tokio::sync::oneshot::error::RecvError,
    },
}

#[derive(Error, Debug)]
pub enum ChannelSendError {
    #[error("Error while sending text message")]
    TextError {
        #[from]
        source: flume::SendError<ClientMessage>,
    },
}
