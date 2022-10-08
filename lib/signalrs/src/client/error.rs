use thiserror::Error;

use super::messages::ClientMessage;

#[derive(Error, Debug)]
pub enum SignalRClientError {
    #[error("Json error")]
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
}

#[derive(Error, Debug)]
pub enum ChannelSendError {
    #[error("Error while sending text message")]
    TextError {
        #[from]
        source: flume::SendError<ClientMessage>,
    },
}
