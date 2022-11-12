use thiserror::Error;
use crate::messages::SerializationError;
use super::invocation::ExtractionError;

#[derive(Debug, Error)]
pub enum HubError {
    /// Container for all user-defined errors
    #[error("an error occured: {message}")]
    Generic { message: String },

    /// Extraction of data from request failed
    ///
    /// This can be argument deserialization or extracion of user defined types
    #[error("error during extraction from invocation")]
    Extraction {
        #[from]
        source: ExtractionError,
    },

    /// Hub does not support features requested
    #[error("{message}")]
    Unsupported { message: String },

    /// Request was well-formed syntactically, but cannot be processed
    #[error("hub understood reuqtes, but cannot process it due to {message}")]
    Unprocessable { message: String },

    /// Occurs when request cannot be understood
    ///
    /// Examples are malformed JSON or bad data format
    #[error("hub could not understand the request")]
    Incomprehensible {
        #[from]
        source: MalformedRequest,
    },
}

#[derive(Debug, Error)]
pub enum MalformedRequest {
    #[error("{source}")]
    Serialization {
        #[from]
        source: SerializationError,
    },
}
