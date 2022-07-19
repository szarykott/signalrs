use thiserror::Error;

use crate::response::HubResponseStruct;

#[derive(Error, Debug)]
pub enum SignalRError {
    #[error("JSON deserialization error")]
    JsonError(#[from] serde_json::Error),
    #[error("Channel error")]
    ChannelError(#[from] flume::SendError<HubResponseStruct>),
    #[error("Unspecified error")]
    UnnspecifiedError,
}
