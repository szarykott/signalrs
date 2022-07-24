use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::error::SignalRError;

use self::{inflight_invocations::InflightInvocations, upload_sinks::UploadSinks};

pub mod inflight_invocations;
pub mod upload_sinks;

#[derive(Default, Clone)] // TODO: Is clone really needed?!
pub struct ConnectionState {
    pub(crate) inflight_invocations: InflightInvocations,
    pub(crate) client_streams_mapping: UploadSinks,
}

pub enum StreamItemPayload {
    Text(Value),
    Binary,
}

impl StreamItemPayload {
    pub fn try_deserialize<T>(self) -> Result<T, SignalRError>
    where
        T: DeserializeOwned,
    {
        match self {
            StreamItemPayload::Text(text) => serde_json::from_value(text).map_err(|e| e.into()),
            StreamItemPayload::Binary => unimplemented!(),
        }
    }
}
