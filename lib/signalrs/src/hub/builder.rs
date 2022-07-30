use super::Hub;
use crate::{
    error::SignalRError,
    functions::{
        Callable, Handler, HandlerV2, IntoCallable, NonStreamingCallable, StreamingCallable,
        StreamingHandlerV2,
    },
};
use futures::Future;
use std::{collections::HashMap, pin::Pin, sync::Arc};

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
        Args: Send + Sync + 'static,
    {
        let callable: IntoCallable<_, Args> = IntoCallable::new(handler, false);
        self.methods.insert(name.to_owned(), Arc::new(callable));
        self
    }

    pub fn methodv2<H, Args>(mut self, name: &str, handler: H) -> Self
    where
        H: HandlerV2<Args, Future = Pin<Box<dyn Future<Output = Result<(), SignalRError>> + Send>>>
            + 'static
            + Clone
            + Send
            + Sync,
        Args: Send + Sync + 'static,
    {
        let callable: NonStreamingCallable<_, Args> = NonStreamingCallable::new(handler);
        self.methods.insert(name.to_owned(), Arc::new(callable));
        self
    }

    pub fn streaming_method<H, Args>(mut self, name: &str, handler: H) -> Self
    where
        H: Handler<Args, Future = Pin<Box<dyn Future<Output = Result<(), SignalRError>> + Send>>>
            + 'static
            + Clone
            + Send
            + Sync,
        Args: Send + Sync + 'static,
    {
        let callable: IntoCallable<_, Args> = IntoCallable::new(handler, true);
        self.methods.insert(name.to_owned(), Arc::new(callable));
        self
    }

    pub fn streaming_methodv2<H, Args>(mut self, name: &str, handler: H) -> Self
    where
        H: StreamingHandlerV2<
                Args,
                Future = Pin<Box<dyn Future<Output = Result<(), SignalRError>> + Send>>,
            >
            + 'static
            + Clone
            + Send
            + Sync,
        Args: Send + Sync + 'static,
    {
        let callable: StreamingCallable<_, Args> = StreamingCallable::new(handler);
        self.methods.insert(name.to_owned(), Arc::new(callable));
        self
    }

    pub fn build(self) -> Hub {
        Hub {
            methods: self.methods,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::HubBuilder;
    use crate::extract::Args;
    use async_stream::stream;
    use futures::Stream;

    #[test]
    fn test() {
        let hub = HubBuilder::new()
            .method("identity", identity)
            .methodv2("noop", noop)
            .method("identity2", identity)
            .streaming_methodv2("noop_stream", noop_stream)
            .build();

        assert!(hub.methods.len() == 4);
    }

    pub async fn identity(Args(a): Args<i32>) -> i32 {
        a
    }

    pub async fn noop() {}

    pub async fn noop_stream() -> impl Stream<Item = ()> {
        stream! {
            yield ();
        }
    }
}
