use std::{collections::HashMap, sync::Arc};

use tokio::sync::Mutex;

#[derive(Debug)]
pub struct HubRequest {
    pub payload: Payload,
    pub hub_state: HubState,
}

impl HubRequest {
    pub fn text(
        payload: String,
        inflight_invocations: Arc<Mutex<HashMap<String, tokio::task::JoinHandle<()>>>>,
    ) -> Self {
        HubRequest {
            payload: Payload::Text(payload),
            hub_state: HubState {
                inflight_invocations,
            },
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum Payload {
    Text(String),
    Binary(Vec<u8>),
}

#[derive(Debug)]
pub struct HubState {
    pub inflight_invocations: Arc<Mutex<HashMap<String, tokio::task::JoinHandle<()>>>>,
}

impl HubRequest {
    pub fn unwrap_text(&self) -> String {
        match &self.payload {
            Payload::Text(v) => v.clone(),
            _ => unimplemented!(),
        }
    }
}
