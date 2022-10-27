#![allow(unused_imports)]
use std::{
    error::Error,
    sync::{Arc, Mutex},
};

use crate::common::SerializeExt;
use async_stream::stream;
use common::{ClientOutputWrapper, TestReceiver};
use flume::{r#async::RecvStream, Receiver, Sender};
use futures::{SinkExt, Stream, StreamExt};
use signalrs::{
    client::{self, ChannelSendError, ClientMessage, SignalRClient, SignalRClientError},
    protocol::*,
    server::{
        connection::ConnectionState,
        hub::{builder::HubBuilder, Hub},
        response::ResponseSink,
        Server,
    },
};

mod common;

// tests inspired by https://github.com/dotnet/aspnetcore/blob/main/src/SignalR/docs/specs/HubProtocol.md#example

#[tokio::test]
async fn test_non_blocking() -> Result<(), Box<dyn Error>> {
    static SHARED: Mutex<usize> = Mutex::new(0usize);

    async fn non_blocking(a: usize, b: usize) {
        let mut num = SHARED.lock().unwrap();
        *num = a + b;
    }

    let mut client =
        get_wired_client(HubBuilder::new().method(stringify!(non_blocking), non_blocking));

    // well, no error is quite ok here
    client
        .method(stringify!(non_blocking))
        .arg(1i32)?
        .arg(2i32)?
        .send()
        .await?;

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    let num = SHARED.lock().unwrap();
    assert_eq!(3, *num);

    Ok(())
}

#[tokio::test]
async fn test_add() -> Result<(), Box<dyn Error>> {
    async fn add(a: i32, b: i32) -> i32 {
        a + b
    }

    let mut client = get_wired_client(HubBuilder::new().method(stringify!(add), add));

    let result: i32 = client
        .method(stringify!(add))
        .arg(1i32)?
        .arg(2i32)?
        .invoke()
        .await?;

    assert_eq!(3, result);

    Ok(())
}

#[tokio::test]
async fn test_stream() -> Result<(), Box<dyn Error>> {
    async fn stream(count: usize) -> impl Stream<Item = usize> {
        stream! {
            for i in 0..count {
                yield i;
            }
        }
    }

    let mut client =
        get_wired_client(HubBuilder::new().streaming_method(stringify!(stream), stream));

    let mut result = client
        .method(stringify!(stream))
        .arg(5usize)?
        .invoke_stream::<usize>()
        .await?;

    assert_eq!(0usize, result.next().await.unwrap().unwrap());
    assert_eq!(1usize, result.next().await.unwrap().unwrap());
    assert_eq!(2usize, result.next().await.unwrap().unwrap());
    assert_eq!(3usize, result.next().await.unwrap().unwrap());
    assert_eq!(4usize, result.next().await.unwrap().unwrap());
    assert_eq!(true, result.next().await.is_none());

    Ok(())
}

// ============== HELPERS ======================== //

fn get_wired_client(
    hub_builder: HubBuilder,
) -> SignalRClient<ClientOutputWrapper<ClientMessage>, RecvStream<'static, ClientMessage>> {
    let (hub_tx, hub_rx) = common::create_channels();
    let hub: Server = hub_builder.build().into();

    let (client, client_tx, client_rx) = build_client();

    let f1 = async move {
        let state = ConnectionState::default();
        while let Ok(ClientMessage::Json(next)) = client_rx.recv_async().await {
            hub.invoke_text(next.to_string(), state.clone(), hub_tx.clone())
                .await
                .unwrap();
        }
    };

    let f2 = async move {
        let mut hub_rx = hub_rx.into_text_stream();
        while let Some(next) = hub_rx.next().await {
            client_tx
                .send_async(ClientMessage::Json(next))
                .await
                .unwrap();
        }
    };

    tokio::spawn(f1);
    tokio::spawn(f2);

    client
}

fn build_client() -> (
    SignalRClient<ClientOutputWrapper<ClientMessage>, RecvStream<'static, ClientMessage>>,
    Sender<ClientMessage>,
    Receiver<ClientMessage>,
) {
    let (client_output_sender, client_output_receiver) = flume::bounded::<ClientMessage>(100);
    let (client_input_sender, client_input_receiver) = flume::bounded::<ClientMessage>(100);

    let client_output_sender =
        ClientOutputWrapper::<ClientMessage>::new_text(client_output_sender.into_sink());

    let client = client::new_text_client(client_output_sender, client_input_receiver.into_stream());

    (client, client_input_sender, client_output_receiver)
}
