use async_mutex::Mutex;
use async_stream::stream;
use futures::{Sink, SinkExt, Stream, StreamExt};
use serde;
use serde::Deserialize;
use signalrs_core::{extensions::StreamExtR, protocol::*};
use std::{any::Any, collections::HashMap, fmt::Debug, sync::Arc};

const WEIRD_ENDING: &'static str = "\u{001E}";

pub struct HubInvoker {
    hub: Arc<Hub>,
    ongoing_invocations: Arc<Mutex<HashMap<String, Box<dyn Any + Send>>>>,
}

#[derive(Deserialize, Debug, Clone, Copy)]
struct Type {
    #[serde(rename = "type")]
    message_type: MessageType,
}

#[derive(Deserialize, Debug, Clone)]
struct Target {
    target: String,
}

impl HubInvoker {
    pub fn new() -> Self {
        HubInvoker {
            hub: Arc::new(Hub {
                _counter: Arc::new(Mutex::new(0)),
            }),
            ongoing_invocations: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn handshake(&self, input: &str) -> String {
        let input = input.trim_end_matches("\u{001E}");

        let request = serde_json::from_str::<HandshakeRequest>(&input);

        let response = match request {
            Ok(request) => {
                if request.is_json() {
                    HandshakeResponse::no_error()
                } else {
                    HandshakeResponse::error("Unsupported protocol")
                }
            }
            Err(e) => HandshakeResponse::error(e),
        };

        match serde_json::to_string(&response) {
            Ok(value) => format!("{}\u{001E}", value),
            Err(e) => "serialization error".to_string(),
        }
    }

    pub async fn invoke_binary(&self, _data: &[u8]) -> Vec<u8> {
        unimplemented!()
    }

    pub async fn invoke_text<T>(&self, text: String, mut output: T)
    where
        T: Sink<String> + Send + 'static + Unpin,
        <T as Sink<String>>::Error: Debug,
    {
        let text = text.trim_end_matches("\u{001E}").to_owned();

        let message_type: Type = serde_json::from_str(&text).unwrap();

        match message_type.message_type {
            MessageType::Invocation => {
                let target: Target = serde_json::from_str(&text).unwrap();
                match target.target.as_str() {
                    "non_blocking" => {
                        self.hub.non_blocking();
                    }
                    "add" => {
                        let mut invocation: Invocation<AddArgs> =
                            serde_json::from_str(&text).unwrap();
                        let arguments = invocation.arguments().unwrap();
                        let result = self.hub.add(arguments.0, arguments.1);
                        match invocation.id() {
                            Some(id) => {
                                let return_message =
                                    Completion::new(id.clone(), Some(result), None);
                                output
                                    .send(
                                        serde_json::to_string(&return_message).unwrap()
                                            + WEIRD_ENDING,
                                    )
                                    .await
                                    .unwrap();
                            }
                            None => {}
                        }
                    }
                    "single_result_failure" => {
                        let mut invocation: Invocation<SingleResultFailureArgs> =
                            serde_json::from_str(&text).unwrap();

                        let arguments = invocation.arguments().unwrap();

                        let result = self.hub.single_result_failure(arguments.0, arguments.1);

                        match (invocation.id(), result) {
                            (Some(id), Ok(result)) => {
                                let return_message =
                                    Completion::new(id.clone(), Some(result), None);
                                output
                                    .send(
                                        serde_json::to_string(&return_message).unwrap()
                                            + WEIRD_ENDING,
                                    )
                                    .await
                                    .unwrap();
                            }
                            (Some(id), Err(e)) => {
                                let return_message =
                                    Completion::<()>::new(id.clone(), None, Some(e));
                                output
                                    .send(
                                        serde_json::to_string(&return_message).unwrap()
                                            + WEIRD_ENDING,
                                    )
                                    .await
                                    .unwrap();
                            }
                            _ => {}
                        }
                    }
                    "batched" => {
                        let mut invocation: Invocation<BatchedArgs> =
                            serde_json::from_str(&text).unwrap();

                        let arguments = invocation.arguments().unwrap();

                        let result = self.hub.batched(arguments.0);

                        match invocation.id() {
                            Some(id) => {
                                let return_message =
                                    Completion::new(id.clone(), Some(result), None);
                                output
                                    .send(
                                        serde_json::to_string(&return_message).unwrap()
                                            + WEIRD_ENDING,
                                    )
                                    .await
                                    .unwrap();
                            }
                            None => {}
                        }
                    }
                    _ => panic!(),
                }
            }
            MessageType::StreamInvocation => {
                let target: Target = serde_json::from_str(&text).unwrap();
                match target.target.as_str() {
                    "stream" => {
                        let stream_invocation: StreamInvocation<StreamArgs> =
                            serde_json::from_str(&text).unwrap();
                        let arguments = stream_invocation.arguments.unwrap();
                        let invocation_id = stream_invocation.invocation_id;
                        let iidc = invocation_id.clone();

                        let result = self.hub.stream(arguments.0);

                        let responses = result
                            .zip(futures::stream::repeat(invocation_id.clone()))
                            .map(|(e, id)| StreamItem::new(id, e))
                            .map(|si| serde_json::to_string(&si).unwrap() + WEIRD_ENDING)
                            .chain(futures::stream::once(async {
                                let completion: Completion<usize> =
                                    Completion::new(invocation_id, None, None);
                                serde_json::to_string(&completion).unwrap() + WEIRD_ENDING
                            }));

                        let mut responses = Box::pin(responses);
                        let invocations = Arc::clone(&self.ongoing_invocations);

                        let iidc2 = iidc.clone();
                        let ongoing = tokio::spawn(async move {
                            while let Some(item) = responses.next().await {
                                output.send(item).await.unwrap();
                            }
                            let mut invocations = invocations.lock().await;
                            (*invocations).remove(&iidc2);
                        });

                        let mut guard = self.ongoing_invocations.lock().await;
                        (*guard).insert(iidc, Box::new(ongoing));
                    }
                    "stream_failure" => {
                        let stream_invocation: StreamInvocation<StreamFailureArgs> =
                            serde_json::from_str(&text).unwrap();
                        let arguments = stream_invocation.arguments.unwrap();
                        let invocation_id = stream_invocation.invocation_id;
                        let iidc = invocation_id.clone();

                        let result = self.hub.stream_failure(arguments.0);

                        let responses = result
                            .take_while_inclusive(|e| e.is_ok())
                            .zip(futures::stream::repeat(invocation_id.clone()))
                            .map(|(e, id)| -> Result<StreamItem<usize>, Completion<()>> {
                                match e {
                                    Ok(item) => Ok(StreamItem::new(id, item)),
                                    Err(e) => Err(Completion::<()>::new(id, None, Some(e))),
                                }
                            })
                            .chain_if(
                                |e| e.is_ok(),
                                futures::stream::once(async {
                                    let r: Result<StreamItem<usize>, Completion<()>> =
                                        Err(Completion::<()>::new(invocation_id, None, None));
                                    r
                                }),
                            )
                            .map(|e| match e {
                                Ok(si) => serde_json::to_string(&si).unwrap() + WEIRD_ENDING,
                                Err(cmp) => serde_json::to_string(&cmp).unwrap() + WEIRD_ENDING,
                            });

                        let mut responses = Box::pin(responses);
                        let invocations = Arc::clone(&self.ongoing_invocations);

                        let iidc2 = iidc.clone();
                        let ongoing = tokio::spawn(async move {
                            while let Some(item) = responses.next().await {
                                output.send(item).await.unwrap();
                            }
                            let mut invocations = invocations.lock().await;
                            (*invocations).remove(&iidc2);
                        });

                        let mut guard = self.ongoing_invocations.lock().await;
                        (*guard).insert(iidc, Box::new(ongoing));
                    }
                    _ => panic!(),
                }
            }
            MessageType::StreamItem => todo!(),
            MessageType::Completion => todo!(),
            MessageType::CancelInvocation => todo!(),
            MessageType::Ping => {
                let ping = Ping::new();
                let s = serde_json::to_string(&ping).unwrap();
                output.send(s + WEIRD_ENDING).await.unwrap();
            }
            MessageType::Close => todo!(),
            MessageType::Other => todo!(),
        }
    }
}

pub struct Hub {
    _counter: Arc<Mutex<usize>>,
}

impl Hub {
    pub fn non_blocking(&self) {
        // nothing
    }

    pub fn add(&self, a: u32, b: u32) -> u32 {
        a + b
    }

    pub fn single_result_failure(&self, _a: u32, _b: u32) -> Result<u32, String> {
        Err("An error!".to_string())
    }

    pub fn batched(&self, count: usize) -> Vec<usize> {
        std::iter::successors(Some(0usize), |p| Some(p + 1))
            .take(count)
            .collect()
    }

    pub fn stream(&self, count: usize) -> impl Stream<Item = usize> {
        stream! {
            for i in 0..count {
                yield i;
            }
        }
    }

    // TODO: Break the stream on error?
    pub fn stream_failure(&self, count: usize) -> impl Stream<Item = Result<usize, String>> {
        stream! {
            for i in 0..count {
                yield Ok(i);
            }
            yield Err("Ran out of data!".to_string())
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
struct BatchedArgs(usize, #[serde(default)] ());

#[derive(Deserialize, Clone, Debug)]
struct AddArgs(u32, u32);

#[derive(Deserialize, Clone, Debug)]
struct SingleResultFailureArgs(u32, u32);

#[derive(Deserialize, Clone, Debug)]
struct StreamArgs(usize, #[serde(default)] ());

#[derive(Deserialize, Clone, Debug)]
struct StreamFailureArgs(usize, #[serde(default)] ());
