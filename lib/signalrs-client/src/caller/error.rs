use thiserror::Error;

use crate::messages::SerializationError;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("malformed request")]
    Malformed {
        #[from]
        source: SerializationError,
    },
}
