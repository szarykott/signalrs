use async_stream::stream;
use futures::{future, Stream, StreamExt};
use serde;
use serde::Deserialize;
use signalrs_core::{extensions::StreamExtR, protocol::*};
use std::{
    fmt::Debug,
    pin::Pin,
    sync::{Arc, Mutex},
};

pub struct HubInvoker {
    hub: Hub,
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

pub enum HubResponse<T> {
    Void,
    Single(T),
    Stream(Pin<Box<dyn Stream<Item = T>>>),
}

impl<T> HubResponse<T> {
    pub fn unwrap_single(self) -> T {
        match self {
            HubResponse::Single(v) => v,
            _ => panic!(),
        }
    }

    pub fn unwrap_stream(self) -> Pin<Box<dyn Stream<Item = T>>> {
        match self {
            HubResponse::Stream(stream) => stream,
            _ => panic!(),
        }
    }
}

impl HubInvoker {
    pub fn new() -> Self {
        HubInvoker {
            hub: Hub {
                _counter: Arc::new(Mutex::new(0)),
            },
        }
    }

    pub async fn invoke_binary(&self, _data: &[u8]) -> Vec<u8> {
        vec![0, 1, 1]
    }

    pub async fn invoke_text(&self, text: &str) -> HubResponse<String> {
        let message_type: Type = serde_json::from_str(text).unwrap();

        match message_type.message_type {
            MessageType::Invocation => {
                let target: Target = serde_json::from_str(text).unwrap();
                match target.target.as_str() {
                    "add" => {
                        let mut invocation: Invocation<AddArgs> =
                            serde_json::from_str(text).unwrap();

                        let arguments = invocation.arguments().unwrap();

                        let result = self.hub.add(arguments.0, arguments.1);

                        match invocation.id() {
                            Some(id) => {
                                let return_message =
                                    Completion::new(id.clone(), Some(result), None);
                                HubResponse::Single(serde_json::to_string(&return_message).unwrap())
                            }
                            None => HubResponse::Void,
                        }
                    }
                    "single_result_failure" => {
                        let mut invocation: Invocation<SingleResultFailureArgs> =
                            serde_json::from_str(text).unwrap();

                        let arguments = invocation.arguments().unwrap();

                        let result = self.hub.single_result_failure(arguments.0, arguments.1);

                        match (invocation.id(), result) {
                            (Some(id), Ok(result)) => {
                                let return_message =
                                    Completion::new(id.clone(), Some(result), None);
                                HubResponse::Single(serde_json::to_string(&return_message).unwrap())
                            }
                            (Some(id), Err(e)) => {
                                let return_message =
                                    Completion::<()>::new(id.clone(), None, Some(e));
                                HubResponse::Single(serde_json::to_string(&return_message).unwrap())
                            }
                            _ => HubResponse::Void,
                        }
                    }
                    "batched" => {
                        let mut invocation: Invocation<BatchedArgs> =
                            serde_json::from_str(text).unwrap();

                        let arguments = invocation.arguments().unwrap();

                        let result = self.hub.batched(arguments.0);

                        match invocation.id() {
                            Some(id) => {
                                let return_message =
                                    Completion::new(id.clone(), Some(result), None);
                                HubResponse::Single(serde_json::to_string(&return_message).unwrap())
                            }
                            None => HubResponse::Void,
                        }
                    }
                    _ => panic!(),
                }
            }
            MessageType::StreamInvocation => {
                let target: Target = serde_json::from_str(text).unwrap();
                match target.target.as_str() {
                    "stream" => {
                        let stream_invocation: StreamInvocation<StreamArgs> =
                            serde_json::from_str(text).unwrap();
                        let arguments = stream_invocation.arguments.unwrap();
                        let invocation_id = stream_invocation.invocation_id;

                        let result = self.hub.stream(arguments.0);

                        let responses = result
                            .zip(futures::stream::repeat(invocation_id.clone()))
                            .map(|(e, id)| StreamItem::new(id, e))
                            .map(|si| serde_json::to_string(&si).unwrap())
                            .chain(futures::stream::once(async {
                                let completion: Completion<usize> =
                                    Completion::new(invocation_id, None, None);
                                serde_json::to_string(&completion).unwrap()
                            }));

                        HubResponse::Stream(Box::pin(responses))
                    }
                    "stream_failure" => {
                        let stream_invocation: StreamInvocation<StreamFailureArgs> =
                            serde_json::from_str(text).unwrap();
                        let arguments = stream_invocation.arguments.unwrap();
                        let invocation_id = stream_invocation.invocation_id;

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
                                Ok(si) => serde_json::to_string(&si).unwrap(),
                                Err(cmp) => serde_json::to_string(&cmp).unwrap(),
                            });

                        HubResponse::Stream(Box::pin(responses))
                    }
                    _ => panic!(),
                }
            }
            MessageType::StreamItem => todo!(),
            MessageType::Completion => todo!(),
            MessageType::CancelInvocation => todo!(),
            MessageType::Ping => todo!(),
            MessageType::Close => todo!(),
            MessageType::Other => todo!(),
        }
    }
}

pub struct Hub {
    _counter: Arc<Mutex<usize>>,
}

impl Hub {
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
