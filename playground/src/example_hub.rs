use serde::{Deserialize, Serialize};
use signalrs_core::protocol::*;
use std::convert::TryFrom;

pub struct Hub {
    path: &'static str,
}

impl Hub {
    pub async fn target1(&mut self) {
        todo!()
    }

    pub async fn target2(&mut self, v: i32) -> impl HubResponse {
        return "";
    }
}

pub trait HubResponse: Serialize {}
impl<T> HubResponse for T where T: Serialize {}

#[derive(Deserialize)]
pub struct Target2Args(i32);

pub struct HubInvoker {
    hub: Hub,
    format: MessageFormat,
}

#[derive(Deserialize)]
pub struct Type {
    r#type: Option<MessageType>,
}

impl HubInvoker {
    pub async fn invoke(&mut self, message: &[u8]) {
        let r#type: Type = match self.format {
            MessageFormat::Json => serde_json::from_slice(message).unwrap(),
            MessageFormat::MessagePack => rmp_serde::from_read_ref(message).unwrap(),
        };

        match r#type.r#type.unwrap_or(MessageType::Other) {
            MessageType::Invocation => {
                let m: Invocation<Target2Args> = match self.format {
                    MessageFormat::Json => serde_json::from_slice(message).unwrap(),
                    MessageFormat::MessagePack => rmp_serde::from_read_ref(message).unwrap(),
                };

                match m.target() {
                    "target2" => {
                        self.hub.target2(m.arguments().0).await;
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
