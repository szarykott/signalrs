use alloc::alloc::Global;
use futures::{
    sink::Sink,
    stream::{FuturesUnordered, Map, Stream, StreamExt, StreamFuture},
};
use signalrs_core::protocol::*;
use signalrs_error::SignalRError;
use std::collections::HashMap;

pub struct IncomingClient {
    input: Box<dyn Stream<Item = Vec<u8>> + Unpin>,
    output: Box<dyn Sink<Vec<u8>, Error = SignalRError> + Unpin>,
    format: MessageFormat,
}

pub struct Clients {
    counter: usize,
    inputs: FuturesUnordered<StreamFuture<Map<Box<dyn Stream<Item = (usize, Vec<u8>)> + Unpin>>>>,
}

impl Clients {
    pub async fn push(&mut self, client: IncomingClient) {
        let mapped = client.input.map(|e| (self.counter, e));
        let sf = mapped.into_future();
        self.inputs.push(sf);
    }
}

impl Stream for Clients {
    type Item = (usize, Vec<u8>);

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        todo!()
    }
}
