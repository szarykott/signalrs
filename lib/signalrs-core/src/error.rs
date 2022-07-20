use thiserror::Error;

use crate::{request::StreamItemPayload, response::HubResponseStruct};

#[derive(Error, Debug)]
pub enum SignalRError {
    #[error("JSON deserialization error")]
    JsonError(#[from] serde_json::Error),
    #[error("Channel error")]
    ChannelError(#[from] flume::SendError<HubResponseStruct>),
    #[error("Channel error")]
    ChannelError2(#[from] flume::SendError<StreamItemPayload>),
    #[error("Unspecified error")]
    UnnspecifiedError,
}
