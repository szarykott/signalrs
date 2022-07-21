use std::{collections::HashMap, sync::Arc};

use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::Mutex;

use crate::{error::SignalRError, hub::ClientSink};

pub struct HubInvocation {
    pub payload: Payload,
    pub hub_state: HubState,
    pub pipeline_state: PipelineState,
}

impl HubInvocation {
    pub fn text(
        payload: String,
        inflight_invocations: Arc<Mutex<HashMap<String, tokio::task::JoinHandle<()>>>>,
        client_streams_mapping: Arc<Mutex<HashMap<String, ClientSink>>>,
    ) -> Self {
        HubInvocation {
            payload: Payload::Text(payload),
            hub_state: HubState {
                inflight_invocations,
                client_streams_mapping,
            },
            pipeline_state: Default::default(),
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum Payload {
    Text(String),
    Binary(Vec<u8>),
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

pub struct HubState {
    pub inflight_invocations: Arc<Mutex<HashMap<String, tokio::task::JoinHandle<()>>>>,
    pub client_streams_mapping: Arc<Mutex<HashMap<String, ClientSink>>>,
}

#[derive(Default)]
pub struct PipelineState {
    pub next_stream_id_index: usize,
}

impl HubInvocation {
    pub fn unwrap_text(&self) -> String {
        match &self.payload {
            Payload::Text(v) => v.clone(),
            _ => unimplemented!(),
        }
    }
}
