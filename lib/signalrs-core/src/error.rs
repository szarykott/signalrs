use thiserror::Error;

use crate::{request::StreamItemPayload, response::HubResponseStruct, extract::ExtractionError};

#[derive(Error, Debug)]
pub enum SignalRError {
    #[error("JSON deserialization error")]
    JsonError {
        #[from]
        source: serde_json::Error
    },
    #[error("Channel error")]
    ChannelError {
        #[from] 
        source: flume::SendError<HubResponseStruct>
    },
    #[error("Channel error")]
    ChannelError2{
        #[from] 
        source: flume::SendError<StreamItemPayload>},
    #[error("An error occured during arguments extraction")]
    ExtrationError {
        #[from]
        source: ExtractionError
    },
    #[error("Unspecified error")]
    UnnspecifiedError,
}
