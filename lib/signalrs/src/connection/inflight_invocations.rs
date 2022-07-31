use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Default)]
pub(crate) struct InflightInvocations {
    value: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

impl InflightInvocations {
    pub fn cancel(&self, invocation_id: &str) {
        let mut guard = self.value.lock().unwrap();
        let result = (*guard).remove(invocation_id);

        // be sure to release lock
        drop(guard);

        match result {
            Some(handle) => {
                handle.cancel();
            }
            None => { /* all good */ }
        };
    }

    pub fn remove(&self, invocation_id: &str) {
        let mut guard = self.value.lock().unwrap();
        let _ = (*guard).remove(invocation_id);
    }

    pub fn get(&self, invocation_id: &str) -> Option<CancellationToken> {
        let guard = self.value.lock().unwrap();
        (*guard)
            .get(invocation_id)
            .and_then(|tkn| Some(tkn.clone()))
    }

    pub fn insert_token(&self, invocation_id: String, cancellation_token: CancellationToken) {
        let mut guard = self.value.lock().unwrap();
        (*guard).insert(invocation_id, cancellation_token);
    }
}
