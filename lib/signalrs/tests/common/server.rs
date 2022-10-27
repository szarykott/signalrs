use std::time::Duration;

use flume::Receiver;
use futures::{Stream, StreamExt};
use serde::{de::DeserializeOwned, Serialize};
use signalrs::server::response::{HubResponseStruct, ResponseSink};

pub struct TestReceiver {
    receiver: Receiver<HubResponseStruct>,
}

impl TestReceiver {
    pub async fn receive_text(&self) -> String {
        self.receive_timeout()
            .await
            .unwrap_text()
            .strip_record_separator()
    }

    async fn receive_timeout(&self) -> HubResponseStruct {
        let timeout = tokio::time::timeout(Duration::from_secs(3), self.receiver.recv_async());

        match timeout.await {
            Ok(value) => value.unwrap(),
            Err(e) => panic!("receive timedout with error {}", e),
        }
    }

    pub fn into_text_stream(self) -> impl Stream<Item = String> {
        self.receiver.into_stream().map(|x| x.unwrap_text())
    }

    pub async fn receive_text_into<T>(&self) -> T
    where
        T: DeserializeOwned,
    {
        let text = self.receive_text().await;
        serde_json::from_str(&text).unwrap()
    }

    pub async fn assert_none(&self) {
        tokio::task::yield_now().await;

        let result = self.receiver.try_recv();

        match result {
            Ok(_) => panic!("Expected dropped channel, got message"),
            Err(e) => match e {
                flume::TryRecvError::Empty => panic!("Expected dropped channel, got empty"),
                flume::TryRecvError::Disconnected => { /* ok*/ }
            },
        }
    }
}

pub fn create_channels() -> (ResponseSink, TestReceiver) {
    let (tx, rx) = flume::bounded(25);
    (ResponseSink::new(tx.into_sink()), TestReceiver { receiver: rx })
}

const WEIRD_ENDING: &str = "\u{001E}";

pub trait StringExt {
    fn strip_record_separator(self) -> String;
}

impl StringExt for String {
    fn strip_record_separator(self) -> String {
        self.trim_end_matches(WEIRD_ENDING).to_owned()
    }
}

pub trait SerializeExt {
    fn to_json(&self) -> String;
}

impl<T> SerializeExt for T
where
    T: Serialize,
{
    fn to_json(&self) -> String {
        serde_json::to_string(&self).unwrap()
    }
}
