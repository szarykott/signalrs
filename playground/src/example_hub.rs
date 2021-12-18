use futures::{
    select,
    sink::{Sink, SinkExt},
    stream::{FusedStream, Stream, StreamExt},
    FutureExt,
};
use serde::{Deserialize, Serialize};
use signalrs_core::{extensions::BoxExt, protocol::*};
use signalrs_error::SignalRError;
use std::collections::HashMap;

pub struct Hub {
    counter: usize,
    clients: HashMap<usize, Client>,
    outputs: HashMap<usize, Box<dyn Sink<Vec<u8>, Error = SignalRError>>>,
}

// TODO: Try FuturesUnordered to implement client polling
pub struct Client {
    input: Box<dyn Stream<Item = (usize, Vec<u8>)> + Unpin>,
    output: Box<dyn Sink<Vec<u8>, Error = SignalRError> + Unpin>,
    format: MessageFormat,
}

pub struct IncomingClient {
    input: Box<dyn Stream<Item = Vec<u8>> + Unpin>,
    output: Box<dyn Sink<Vec<u8>, Error = SignalRError> + Unpin>,
    format: MessageFormat,
}

impl IncomingClient {
    fn as_client(self, id: usize) -> Client {
        Client {
            input: self.input.map(move |i| (id, i)).into_box(),
            output: self.output,
            format: self.format,
        }
    }

    async fn initialize(&mut self) {
        let request = self
            .input
            .next()
            .await
            .and_then(|b| Some(self.format.from_bytes::<HandshakeRequest>(&b)));

        if let Some(request) = request {
            self.output
                .send(self.format.to_bytes(&HandshakeResponse::no_error()))
                .await
                .unwrap(); // TODO: Fixme
        }
    }
}

impl Hub {
    pub async fn target1(&mut self) {
        todo!()
    }

    pub async fn target2(&mut self, v: i32) -> impl HubResponse {
        return "";
    }
}

impl Hub {
    pub async fn run<T>(mut self, mut clients_stream: T)
    where
        T: Stream<Item = IncomingClient> + Unpin + FusedStream,
    {
        loop {
            select! {
                new_client = clients_stream.next() => {
                    if let Some(mut new_client) = new_client {
                        new_client.initialize().await;
                        self.clients.insert(self.counter, new_client.as_client(self.counter));
                        self.counter += 1;
                    }
                }
                message = self.poll_clients().fuse() => {
                    if let Some((_id, message)) = message {
                        self.invoke(&message).await
                    }
                }
            }
        }
    }

    async fn poll_clients(&mut self) -> Option<(usize, Vec<u8>)> {
        for (_, client) in self.clients.iter_mut() {
            return client.input.next().await;
        }

        None
    }

    pub async fn invoke(&mut self, message: &[u8]) {
        let r#type: Type = serde_json::from_slice(message).unwrap();

        match r#type.r#type.unwrap_or(MessageType::Other) {
            MessageType::Invocation => {
                let mut invocation: Invocation<Target2Args> =
                    serde_json::from_slice(message).unwrap();

                match invocation.target() {
                    "target1" => self.target1().await,
                    "target2" => {
                        if let Some(args) = invocation.arguments() {
                            self.target2(args.0).await;
                        }
                    }
                    _ => {}
                }
            }
            MessageType::StreamItem => todo!(),
            MessageType::Completion => todo!(),
            MessageType::StreamInvocation => todo!(),
            MessageType::CancelInvocation => todo!(),
            MessageType::Ping => todo!(),
            MessageType::Close => todo!(),
            MessageType::Other => todo!(),
        }
    }
}

pub trait HubResponse: Serialize {}
impl<T> HubResponse for T where T: Serialize {}

#[derive(Deserialize)]
pub struct Target2Args(i32);

#[derive(Deserialize)]
pub struct Type {
    r#type: Option<MessageType>,
}
