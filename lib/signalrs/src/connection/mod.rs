use serde::de::DeserializeOwned;
use serde_json::Value;
use uuid::Uuid;

use crate::error::SignalRError;

pub(crate) use self::{
    inflight_invocations::InflightInvocations,
    upload_sinks::{ClientSink, UploadSinks},
};

mod inflight_invocations;
mod upload_sinks;

#[derive(Clone)] // TODO: Is clone really needed?!
pub struct ConnectionState {
    pub(crate) connection_id: Uuid,
    pub(crate) inflight_invocations: InflightInvocations,
    pub(crate) upload_sinks: UploadSinks,
}

impl Default for ConnectionState {
    fn default() -> Self {
        Self {
            connection_id: Uuid::new_v4(),
            inflight_invocations: Default::default(),
            upload_sinks: Default::default(),
        }
    }
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
