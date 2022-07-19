use super::Hub;
use crate::{
    error::SignalRError,
    handler::{Callable, Handler, IntoCallable},
};
use futures::Future;
use serde::de::DeserializeOwned;
use std::{collections::HashMap, pin::Pin, sync::Arc};

#[allow(missing_debug_implementations)]
pub struct HubBuilder {
    methods: HashMap<
        String,
        Arc<
            dyn Callable<Future = Pin<Box<dyn Future<Output = Result<(), SignalRError>> + Send>>>
                + Send
                + Sync,
        >,
    >,
}

impl HubBuilder {
    pub fn new() -> Self {
        HubBuilder {
            methods: Default::default(),
        }
    }

    pub fn method<H, Args>(mut self, name: &str, handler: H) -> Self
    where
        H: Handler<Args, Future = Pin<Box<dyn Future<Output = Result<(), SignalRError>> + Send>>>
            + 'static
            + Clone
            + Send
            + Sync,
        Args: DeserializeOwned + Send + Sync + 'static,
    {
        let callable: IntoCallable<_, Args> = IntoCallable::new(handler);
        self.methods.insert(name.to_owned(), Arc::new(callable));
        self
    }

    pub fn build(self) -> Hub {
        Hub {
            methods: self.methods,
            inflight_invocations: Default::default(),
            client_streams_mapping: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::extract::Args;

    use super::HubBuilder;

    #[test]
    fn test() {
        let hub = HubBuilder::new()
            .method("identity", identity)
            .method("identity2", identity)
            .build();

        assert!(hub.methods.len() == 2);
    }

    pub async fn identity(Args(a): Args<i32>) -> i32 {
        a
    }
}
