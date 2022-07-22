use std::{collections::HashMap, sync::Arc};

use futures::Future;
use tokio::{sync::Mutex, task::JoinHandle};

#[derive(Debug, Clone)]
pub(crate) struct InflightInvocations {
    value: Arc<Mutex<HashMap<String, JoinHandle<()>>>>,
}

impl InflightInvocations {
    pub async fn cancel(&self, invocation_id: &str) {
        let mut guard = self.value.lock().await;
        match (*guard).remove(invocation_id) {
            Some(handle) => handle.abort(),
            None => { /* all good */ }
        };
    }

    pub async fn remove(&self, invocation_id: &str) {
        let mut guard = self.value.lock().await;
        let _ = (*guard).remove(invocation_id);
    }

    pub async fn insert<F>(&self, invocation_id: String, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let mut guard = self.value.lock().await;

        // future might want to remove itself from map, acquire lock first
        let handle = tokio::spawn(fut);

        (*guard).insert(invocation_id, handle);
    }
}

impl Default for InflightInvocations {
    fn default() -> Self {
        Self {
            value: Default::default(),
        }
    }
}
