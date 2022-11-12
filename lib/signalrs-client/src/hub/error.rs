use super::invocation::ExtractionError;
use crate::messages::SerializationError;
use thiserror::Error;

/// Generic hub error
///
/// Occurs when there is an error during processing of client hub invocation
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

/// The request to hub was malformed
///
/// For instance json was not correct
#[derive(Debug, Error)]
pub enum MalformedRequest {
    #[error(transparent)]
    Serialization {
        #[from]
        source: SerializationError,
    },
}
