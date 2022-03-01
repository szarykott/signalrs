use serde;
use serde::Deserialize;
use signalrs_core::protocol::*;
use std::{
    sync::{Arc, Mutex},
};
use futures::Stream;

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

#[derive(Debug, Clone)]
pub enum HubResponse<T> {
    Void,
    Single(T),
    Stream(SignalRStream<T>),
}

#[derive(Debug, Clone)]
pub struct SignalRStream<T> {
    v: T
}

impl<T> Stream for SignalRStream<T> {
    type Item = T;

    fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
        todo!()
    }
}

impl<T> HubResponse<T> {
    pub fn unwrap_single(self) -> T {
        match self {
            HubResponse::Single(v) => v,
            _ => panic!(),
        }
    }

    pub fn unwrap_stream(self) -> SignalRStream<T> {
        match self {
            HubResponse::Stream(stream) => stream,
            _ => panic!()
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
}

#[derive(Deserialize, Clone, Debug)]
struct BatchedArgs(usize, #[serde(default)] ());

#[derive(Deserialize, Clone, Debug)]
struct AddArgs(u32, u32);

#[derive(Deserialize, Clone, Debug)]
struct SingleResultFailureArgs(u32, u32);
