#![allow(unused_imports)]
use std::sync::Arc;

use async_stream::stream;
use futures::{
    sink::Sink,
    stream::{Stream, StreamExt},
};
use log::LevelFilter;
use log::*;
use signalrs::{
    connection::ConnectionState, error::SignalRError, extract::UploadStream,
    hub::builder::HubBuilder, invocation, protocol::*, response::ResponseSink,
};
use simple_logger::SimpleLogger;

use crate::common::{SerializeExt, StringExt};

mod common;

// tests inspired by https://github.com/dotnet/aspnetcore/blob/main/src/SignalR/docs/specs/HubProtocol.md#example

#[tokio::test]
async fn test_add() {
    async fn add(a: i32, b: i32) -> i32 {
        a + b
    }

    let hub = HubBuilder::new().method("add", add).build();
    let (tx, rx) = common::create_channels();
    let invocation = Invocation::with_id("123", "add", Some((1i32, 2i32))).to_json();

    hub.invoke_text(invocation, Default::default(), tx)
        .await
        .unwrap();

    assert_eq!(Completion::result("123", 3), rx.receive_text_into().await);
}

#[tokio::test]
async fn test_non_blocking() {
    async fn non_blocking(a: i32, b: i32) {
        print!("result is {a} + {b} = {0}", a + b)
    }

    let hub = HubBuilder::new()
        .method("non_blocking", non_blocking)
        .build();
    let (tx, rx) = common::create_channels();
    let invocation = Invocation::without_id("non_blocking", Some((1i32, 2i32))).to_json();

    hub.invoke_text(invocation, Default::default(), tx)
        .await
        .unwrap();

    rx.assert_none().await;
}

#[tokio::test]
async fn test_single_result_failure() {
    const EXPECTED_MESSAGE: &str = "It didn't work!";
    async fn single_result_failure(_: i32, _: i32) -> Result<(), String> {
        Err(EXPECTED_MESSAGE.to_owned())
    }

    let hub = HubBuilder::new()
        .method("single_result_failure", single_result_failure)
        .build();
    let (tx, rx) = common::create_channels();
    let invocation =
        Invocation::with_id("123", "single_result_failure", Some((1i32, 2i32))).to_json();

    hub.invoke_text(invocation, Default::default(), tx)
        .await
        .unwrap();

    assert_eq!(
        Completion::<i32>::error("123", EXPECTED_MESSAGE),
        rx.receive_text_into().await
    );
}

#[tokio::test]
async fn test_batched() {
    async fn batched(count: usize) -> Vec<usize> {
        std::iter::successors(Some(0usize), |p| Some(p + 1))
            .take(count)
            .collect::<Vec<usize>>()
    }

    let hub = HubBuilder::new().method("batched", batched).build();
    let (tx, rx) = common::create_channels();
    let invocation = Invocation::with_id("123", "batched", Some((5usize,))).to_json();

    hub.invoke_text(invocation, Default::default(), tx)
        .await
        .unwrap();

    assert_eq!(
        Completion::result("123", vec![0, 1, 2, 3, 4]),
        rx.receive_text_into().await
    );
}

#[tokio::test]
async fn test_stream() {
    async fn stream(count: usize) -> impl Stream<Item = usize> {
        stream! {
            for i in 0..count {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                yield i;
            }
        }
    }

    let hub = HubBuilder::new().streaming_method("stream", stream).build();

    let invocation = StreamInvocation::new("123", "stream", Some((3usize,))).to_json();

    let (tx, rx) = common::create_channels();

    hub.invoke_text(invocation, Default::default(), tx)
        .await
        .unwrap();

    assert_eq!(StreamItem::new("123", 0usize), rx.receive_text_into().await);
    assert_eq!(StreamItem::new("123", 1usize), rx.receive_text_into().await);
    assert_eq!(StreamItem::new("123", 2usize), rx.receive_text_into().await);
    assert_eq!(Completion::<()>::ok("123"), rx.receive_text_into().await);
    rx.assert_none().await;
}

#[tokio::test]
async fn test_stream_cancel() {
    async fn stream(count: usize) -> impl Stream<Item = usize> {
        stream! {
            for i in 0..count {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                trace!("yielding {}", i);
                yield i;
            }
            trace!("stream finishing");
        }
    }

    let hub = HubBuilder::new().streaming_method("stream", stream).build();
    let (tx, rx) = common::create_channels();
    let state: ConnectionState = Default::default();

    let inv = StreamInvocation::new("123", "stream", Some((3usize,))).to_json();

    hub.invoke_text(inv, state.clone(), tx.clone())
        .await
        .unwrap();

    assert_eq!(StreamItem::new("123", 0usize), rx.receive_text_into().await);

    hub.invoke_text(CancelInvocation::new("123").to_json(), state.clone(), tx)
        .await
        .unwrap();

    assert_eq!(Completion::<()>::ok("123"), rx.receive_text_into().await);
    rx.assert_none().await;
}

#[tokio::test]
async fn test_stream_failure() {
    async fn stream_failure(count: usize) -> impl Stream<Item = Result<usize, String>> {
        stream! {
            for i in 0..count {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                yield Ok(i);
            }
            yield Err("Ran out of data!".to_string())
        }
    }

    let hub = HubBuilder::new()
        .streaming_method("stream_failure", stream_failure)
        .build();

    let invocation = StreamInvocation::new("123", "stream_failure", Some((3usize,))).to_json();

    let (tx, rx) = common::create_channels();

    hub.invoke_text(invocation, Default::default(), tx)
        .await
        .unwrap();

    assert_eq!(StreamItem::new("123", 0usize), rx.receive_text_into().await);
    assert_eq!(StreamItem::new("123", 1usize), rx.receive_text_into().await);
    assert_eq!(StreamItem::new("123", 2usize), rx.receive_text_into().await);
    assert_eq!(
        Completion::<usize>::error("123", "Ran out of data!"),
        rx.receive_text_into().await
    );
    rx.assert_none().await;
}

#[tokio::test]
async fn test_add_stream() {
    pub async fn add_stream(mut input: UploadStream<i32>) -> i32 {
        let mut result = Vec::new();
        trace!("add_stream invoked");
        while let Some(i) = input.next().await {
            trace!("add_stream next item: {i}");
            result.push(i);
        }

        result.into_iter().sum::<i32>()
    }

    let hub = HubBuilder::new().method("add_stream", add_stream).build();
    let state: ConnectionState = Default::default();
    let (tx, rx) = common::create_channels();

    let mut invocation = Invocation::<()>::with_id("123", "add_stream", None);
    invocation.stream_ids = Some(vec!["1".to_string()]);
    let invocation = invocation.to_json();

    hub.invoke_text(invocation, state.clone(), tx.clone())
        .await
        .unwrap();

    hub.invoke_text(
        StreamItem::new("1", 1i32).to_json(),
        state.clone(),
        tx.clone(),
    )
    .await
    .unwrap();

    hub.invoke_text(
        StreamItem::new("1", 1i32).to_json(),
        state.clone(),
        tx.clone(),
    )
    .await
    .unwrap();

    hub.invoke_text(
        StreamItem::new("1", 1i32).to_json(),
        state.clone(),
        tx.clone(),
    )
    .await
    .unwrap();

    hub.invoke_text(
        Completion::<i32>::ok("1").to_json(),
        state.clone(),
        tx.clone(),
    )
    .await
    .unwrap();

    assert_eq!(Completion::result("123", 3), rx.receive_text_into().await);
}
