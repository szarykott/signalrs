use thiserror::Error;

use crate::{messages::SerializationError, hub::error::HubError};

#[derive(Debug, Error)]
pub enum ClientError {
    /// Issued request was malformed
    #[error("malformed {direction}")]
    Malformed {
        direction: &'static str,
        #[source]
        source: SerializationError,
    },

    /// There was an error during invocation on client-side hub
    #[error(transparent)]
    Hub {
        #[from]
        source: HubError
    },

    /// SignalR protocol was violated
    /// 
    /// Probably by callee, see message for details.
    #[error("protocol violation: {message}")]
    ProtocolError {
        message: String
    },

    /// There was an error 
    #[error("it is no longer possible to receive response: {message}")]
    NoResponse {
        message: String
    },

    /// Error returned from the server
    #[error("error returned from the server: {message}")]
    Result {
        message: String
    },

    /// Client cannot reach transport 
    /// 
    /// There could be abrupt close of underlying transport (WebSockets or other)
    #[error("transport layer is inavailable")]
    TransportInavailable {
        message: String
    },

    #[error("handshake error")]
    Handshake {
        message: String
    }
}

impl ClientError {
    pub fn protocol_violation(message: impl ToString) -> ClientError {
        ClientError::ProtocolError { message: message.to_string() }
    }

    pub fn no_response(message: impl ToString) -> ClientError {
        ClientError::NoResponse { message: message.to_string() }
    }

    pub fn malformed_request(source: SerializationError) -> ClientError {
        ClientError::Malformed { direction: "request", source }
    }

    pub fn malformed_response(source: SerializationError) -> ClientError {
        ClientError::Malformed { direction: "request", source }
    }

    pub fn result(message: impl ToString) -> ClientError {
        ClientError::Result { message: message.to_string() }
    }

    pub fn transport(message: impl ToString) -> ClientError {
        ClientError::TransportInavailable { message: message.to_string() }
    }

    pub fn handshake(message: impl ToString) -> ClientError {
        ClientError::Handshake { message: message.to_string() }
    }
}