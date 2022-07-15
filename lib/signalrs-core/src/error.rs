use thiserror::Error;

#[derive(Error, Debug)]
pub enum SignalRError {
    #[error("JSON deserialization error")]
    JsonError(#[from] serde_json::Error),
}
