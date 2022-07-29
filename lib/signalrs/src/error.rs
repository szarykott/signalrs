use thiserror::Error;

use crate::{connection::StreamItemPayload, extract::ExtractionError, response::HubResponseStruct};

#[derive(Error, Debug)]
pub enum SignalRError {
    #[error("Json error")]
    JsonError {
        #[from]
        source: serde_json::Error,
    },
    #[error("Internal communication error")]
    InternalCommuncationError {
        #[from]
        source: InternalCommuncationError,
    },
    #[error("An error occured during arguments extraction")]
    ExtrationError {
        #[from]
        source: ExtractionError,
    },
}

#[derive(Error, Debug)]
pub enum InternalCommuncationError {
    #[error("An error occured while forwarding hub response")]
    HubResponse {
        #[from]
        source: flume::SendError<HubResponseStruct>,
    },
    #[error("An error occured while handling upload stream item")]
    StreamItem {
        #[from]
        source: flume::SendError<StreamItemPayload>,
    },
}
