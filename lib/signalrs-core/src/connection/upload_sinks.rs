use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::hub::client_sink::ClientSink;

#[derive(Clone, Default)]
pub struct UploadSinks {
    value: Arc<Mutex<HashMap<String, ClientSink>>>,
}

impl UploadSinks {
    pub fn insert(&self, stream_id: String, sink: ClientSink) {
        let mut mappings = self.value.lock().unwrap();
        (*mappings).insert(stream_id, sink);
    }

    pub fn get_sink(&self, stream_id: &String) -> Option<ClientSink> {
        let mappings = self.value.lock().unwrap();
        (*mappings).get(stream_id).and_then(|x| Some(x.clone()))
    }

    pub fn remove(&self, stream_id: &String) {
        let mut mappings = self.value.lock().unwrap();
        (*mappings).remove(stream_id);
    }
}
