#![allow(unused_imports)]
use crate::common::SerializeExt;
use common::{ClientOutputWrapper, TestReceiver};
use flume::{r#async::RecvStream, Receiver, Sender};
use futures::{SinkExt, StreamExt};
use signalrs::{
    client::{self, ChannelSendError, SignalRClient, SignalRClientError},
    connection::ConnectionState,
    hub::{builder::HubBuilder, Hub},
    protocol::*,
    response::ResponseSink,
};

mod common;

// tests inspired by https://github.com/dotnet/aspnetcore/blob/main/src/SignalR/docs/specs/HubProtocol.md#example

#[tokio::test]
async fn test_add() {
    let (hub, hub_tx, hub_rx) = build_hub();
    let (client, client_tx, client_rx) = build_client();

    let f1 = async move {
        let state = ConnectionState::default();
        while let Ok(next) = client_rx.recv_async().await {
            hub.invoke_text(next, state.clone(), hub_tx.clone())
                .await
                .unwrap();
        }
    };

    let f2 = async move {
        let mut hub_rx = hub_rx.into_text_stream();
        while let Some(next) = hub_rx.next().await {
            client_tx.send_async(next).await.unwrap();
        }
    };

    tokio::spawn(f1);
    tokio::spawn(f2);

    // let invocation = Invocation::with_id("123", "add", Some((1i32, 2i32))).to_json();

    // hub.invoke_text(invocation, Default::default(), tx)
    //     .await
    //     .unwrap();

    // assert_eq!(Completion::result("123", 3), rx.receive_text_into().await);
}

fn build_hub() -> (Hub, ResponseSink, TestReceiver) {
    async fn add(a: i32, b: i32) -> i32 {
        a + b
    }

    let hub = HubBuilder::new().method("add", add).build();

    let (tx, rx) = common::create_channels();

    (hub, tx, rx)
}

fn build_client() -> (
    SignalRClient<ClientOutputWrapper<String>, RecvStream<'static, String>, String>,
    Sender<String>,
    Receiver<String>,
) {
    let (client_output_sender, client_output_receiver) = flume::bounded::<String>(100);
    let (client_input_sender, client_input_receiver) = flume::bounded::<String>(100);

    let client_output_sender =
        ClientOutputWrapper::<String>::new_text(client_output_sender.into_sink());

    let client = client::new_text_client(client_output_sender, client_input_receiver.into_stream());

    (client, client_input_sender, client_output_receiver)
}
