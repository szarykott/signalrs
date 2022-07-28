use std::time::Duration;

use flume::Receiver;
use futures::{Stream, StreamExt};
use serde::{de::DeserializeOwned, Serialize};
use signalrs_core::response::{HubResponseStruct, ResponseSink};

pub struct TestReceiver {
    receiver: Receiver<HubResponseStruct>,
}

impl TestReceiver {
    pub fn receive_text(&self) -> String {
        self.receiver
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .unwrap_text()
            .strip_record_separator()
    }

    pub fn receive_text_into<T>(&self) -> T
    where
        T: DeserializeOwned,
    {
        let text = self.receive_text();
        serde_json::from_str(&text).unwrap()
    }

    pub fn assert_none(&self) {
        let result = self.receiver.try_recv();

        match result {
            Ok(_) => panic!("Expected dropped channel"),
            Err(e) => match e {
                flume::TryRecvError::Empty => panic!("Expected dropped channel"),
                flume::TryRecvError::Disconnected => { /* ok*/ }
            },
        }
    }
}

pub fn create_channels() -> (ResponseSink, TestReceiver) {
    let (tx, rx) = flume::bounded(25);
    (
        ResponseSink::new(tx.into_sink()),
        TestReceiver { receiver: rx },
    )
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
