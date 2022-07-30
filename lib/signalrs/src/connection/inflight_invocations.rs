use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use log::*;

use futures::Future;
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

    pub fn insert<F>(&self, invocation_id: String, fut: F, cancellation_token: CancellationToken)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let mut guard = self.value.lock().unwrap();

        let this = self.clone();
        let id = invocation_id.clone();
        tokio::spawn(async move {
            fut.await;
            this.remove(&id);
        });

        (*guard).insert(invocation_id, cancellation_token);
    }
}
