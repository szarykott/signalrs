use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use futures::Future;
use tokio::task::JoinHandle;

#[derive(Debug, Clone, Default)]
pub(crate) struct InflightInvocations {
    value: Arc<Mutex<HashMap<String, JoinHandle<()>>>>,
}

impl InflightInvocations {
    pub fn cancel(&self, invocation_id: &str) {
        let mut guard = self.value.lock().unwrap();
        let result = (*guard).remove(invocation_id);

        // be sure to release lock
        drop(guard);

        match result {
            Some(handle) => handle.abort(),
            None => { /* all good */ }
        };
    }

    pub fn remove(&self, invocation_id: &str) {
        let mut guard = self.value.lock().unwrap();
        let _ = (*guard).remove(invocation_id);
    }

    pub fn insert<F>(&self, invocation_id: String, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let mut guard = self.value.lock().unwrap();

        // future might want to remove itself from map, acquire lock first
        let handle = tokio::spawn(fut);

        (*guard).insert(invocation_id, handle);
    }
}
