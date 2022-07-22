use std::{collections::HashMap, sync::Arc};

use tokio::sync::Mutex;

use super::client_sink::ClientSink;

#[derive(Clone)]
pub struct ClientStreamMap {
    value: Arc<Mutex<HashMap<String, ClientSink>>>,
}

impl ClientStreamMap {
    pub async fn insert(&self, stream_id: String, sink: ClientSink) {
        let mut mappings = self.value.lock().await;
        (*mappings).insert(stream_id, sink);
    }
}
