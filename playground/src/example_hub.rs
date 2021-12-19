use serde;
use serde::Deserialize;
use signalrs_core::protocol::*;
use std::sync::{Arc, Mutex};

pub struct HubInvoker {
    hub: Hub,
}

#[derive(Deserialize, Debug, Clone, Copy)]
struct Type {
    #[serde(rename = "type")]
    message_type: MessageType,
}

pub enum HubResponse<T> {
    Void,
    Single(T),
    Stream
}

impl HubInvoker {
    pub fn new() -> Self {
        HubInvoker {
            hub: Hub {
                counter: Arc::new(Mutex::new(0)),
            },
        }
    }

    pub async fn invoke_binary(&self, _data: &[u8]) -> Vec<u8> {
        vec![0, 1, 1]
    }

    pub async fn invoke_text(&self, text: &str) -> HubResponse<String> {
        let message_type: Type = serde_json::from_str(text).unwrap();

        dbg!(message_type);

        match message_type.message_type {
            MessageType::Invocation => {
                let mut invocation: Invocation<Target2Args> = serde_json::from_str(text).unwrap();

                dbg!(invocation.clone());

                let arguments = invocation.arguments().unwrap();

                let result = self.hub.target2(arguments.0);

                match invocation.id() {
                    Some(id) => {
                        let return_message = Completion::new(id.clone(), Some(result), None);
                        HubResponse::Single(serde_json::to_string(&return_message).unwrap())
                    }, 
                    None => HubResponse::Void
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
    counter: Arc<Mutex<usize>>,
}

impl Hub {
    pub fn target2(&self, v: usize) -> String {
        let mut counter = self.counter.lock().unwrap();
        let new_counter = *counter + v;
        *counter = new_counter;
        format!("{}", new_counter)
    }
}

#[derive(Deserialize, Clone, Debug)]
struct Target2Args(usize, #[serde(default)] ());
