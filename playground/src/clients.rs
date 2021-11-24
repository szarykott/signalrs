use futures::{
    sink::Sink,
    stream::{FuturesUnordered, Stream, StreamExt, StreamFuture, SelectAll},
};
use signalrs_core::{protocol::*, extensions::BoxExt};
use signalrs_error::SignalRError;

pub struct IncomingClient<St> {
    input: Box<dyn Stream<Item = Vec<u8>> + Unpin>,
    output: Box<dyn Sink<Vec<u8>, Error = SignalRError> + Unpin>,
    format: MessageFormat,
    data: St
}

impl IncomingClient<()> {
    fn add_number(self, number: usize) -> IncomingClient<usize> {
        IncomingClient {
            data: number,
            input: self.input,
            output: self.output,
            format: self.format,
        }
    }
}

pub struct Clients {
    counter: usize,
    inputs: SelectAll<Box<dyn Stream<Item = (usize, Vec<u8>)> + Unpin>>,
}

impl Clients {
    pub async fn push(&mut self, client: IncomingClient<usize>) {
        let new_number = self.increment_number();
        let mapped = client.input.map(move |d| (new_number, d));
        self.inputs.push(mapped.into_box());
    }

    fn increment_number(&mut self) -> usize {
        let prev = self.counter;
        self.counter += 1;
        prev
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
