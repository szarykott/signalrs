#![allow(unused_imports)]
use async_stream::stream;
use futures::{
    sink::Sink,
    stream::{Stream, StreamExt},
};
use signalrs_core::{
    error::SignalRError,
    extract::Args,
    hub::builder::HubBuilder,
    protocol::*,
    response::{HubResponse, HubStream, ResponseSink},
};

use crate::common::{SerializeExt, StringExt};

mod common;

// tests inspired by https://github.com/dotnet/aspnetcore/blob/main/src/SignalR/docs/specs/HubProtocol.md#example

#[tokio::test]
async fn test_add() {
    async fn add(Args((a, b)): Args<(i32, i32)>) -> i32 {
        a + b
    }

    let hub = HubBuilder::new().method("add", add).build();
    let (tx, rx) = common::create_channels();
    let invocation = Invocation::with_id("123", "add", Some((1i32, 2i32))).to_json();

    hub.invoke_text(invocation, Default::default(), tx)
        .await
        .unwrap();

    assert_eq!(Completion::with_result("123", 3), rx.receive_text_into());
}

#[tokio::test]
async fn test_single_result_failure() {
    const EXPECTED_MESSAGE: &str = "It didn't work!";
    async fn single_result_failure(Args((_, _)): Args<(i32, i32)>) -> Result<(), String> {
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
        rx.receive_text_into()
    );
}

#[tokio::test]
async fn test_batched() {
    async fn batched(Args(count): Args<usize>) -> Vec<usize> {
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
        Completion::with_result("123", vec![0, 1, 2, 3, 4]),
        rx.receive_text_into()
    );
}

#[tokio::test]
async fn test_stream() {
    async fn stream(Args(count): Args<usize>) -> impl HubResponse {
        HubStream::infallible(stream! {
            for i in 0..count {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                yield i;
            }
        })
    }

    let hub = HubBuilder::new().method("stream", stream).build();

    let invocation = StreamInvocation::new("123", "stream", Some((3usize,))).to_json();

    let (tx, rx) = common::create_channels();

    hub.invoke_text(invocation, Default::default(), tx)
        .await
        .unwrap();

    assert_eq!(StreamItem::new("123", 0usize), rx.receive_text_into());
    assert_eq!(StreamItem::new("123", 1usize), rx.receive_text_into());
    assert_eq!(StreamItem::new("123", 2usize), rx.receive_text_into());
    assert_eq!(Completion::<usize>::ok("123"), rx.receive_text_into());
    rx.assert_none();
}

#[tokio::test]
async fn test_stream_failure() {
    async fn stream_failure(Args(count): Args<usize>) -> impl HubResponse {
        HubStream::fallible(stream! {
            for i in 0..count {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                yield Ok(i);
            }
            yield Err("Ran out of data!".to_string())
        })
    }

    let hub = HubBuilder::new()
        .method("stream_failure", stream_failure)
        .build();

    let invocation = StreamInvocation::new("123", "stream_failure", Some((3usize,))).to_json();

    let (tx, rx) = common::create_channels();

    hub.invoke_text(invocation, Default::default(), tx)
        .await
        .unwrap();

    assert_eq!(StreamItem::new("123", 0usize), rx.receive_text_into());
    assert_eq!(StreamItem::new("123", 1usize), rx.receive_text_into());
    assert_eq!(StreamItem::new("123", 2usize), rx.receive_text_into());
    assert_eq!(
        Completion::<usize>::error("123", "Ran out of data!"),
        rx.receive_text_into()
    );
    rx.assert_none();
}
