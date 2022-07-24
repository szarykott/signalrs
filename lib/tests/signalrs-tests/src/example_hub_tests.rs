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
    protocol,
    response::{HubResponse, HubStream, ResponseSink},
};

const WEIRD_ENDING: &str = "\u{001E}";

#[tokio::test]
async fn simple_invocation_succesfull() {
    async fn add(Args((a, b)): Args<(i32, i32)>) -> i32 {
        a + b
    }

    let hub = HubBuilder::new().method("add", add).build();

    let invocation = protocol::Invocation::new(
        Some("123".to_string()),
        "add".to_string(),
        Some((1i32, 2i32)),
    );

    let request = serde_json::to_string(&invocation).unwrap();

    let (tx, rx) = flume::bounded(1);

    let sink = ResponseSink::new(tx.into_sink());

    hub.invoke_text(request, Default::default(), sink)
        .await
        .unwrap();

    let response = rx.recv().unwrap().unwrap_text().strip_record_separator();
    let response: protocol::Completion<i32> = serde_json::from_str(&response).unwrap();

    let expected_response = protocol::Completion::new("123".to_string(), Some(3), None);

    assert_eq!(expected_response, response);
}

#[tokio::test]
async fn simple_invocation_failed() {
    async fn add(Args((_, _)): Args<(i32, i32)>) -> Result<(), String> {
        Err("An error!".to_owned())
    }

    let hub = HubBuilder::new().method("add", add).build();

    let invocation = protocol::Invocation::new(
        Some("123".to_string()),
        "add".to_string(),
        Some((1i32, 2i32)),
    );

    let request = serde_json::to_string(&invocation).unwrap();

    let (tx, rx) = flume::bounded(1);

    let sink = ResponseSink::new(tx.into_sink());

    hub.invoke_text(request, Default::default(), sink)
        .await
        .unwrap();

    let response = rx.recv().unwrap().unwrap_text().strip_record_separator();
    let response: protocol::Completion<i32> = serde_json::from_str(&response).unwrap();

    let expected_response =
        protocol::Completion::new("123".to_string(), None, Some("An error!".to_string()));

    assert_eq!(expected_response, response);
}

#[tokio::test]
async fn batched_invocation() {
    async fn batched(Args(count): Args<usize>) -> Vec<usize> {
        std::iter::successors(Some(0usize), |p| Some(p + 1))
            .take(count)
            .collect::<Vec<usize>>()
    }

    let hub = HubBuilder::new().method("batched", batched).build();

    let invocation = protocol::Invocation::new(
        Some("123".to_string()),
        "batched".to_string(),
        Some((5usize,)),
    );

    let request = serde_json::to_string(&invocation).unwrap();

    let (tx, rx) = flume::bounded(1);

    let sink = ResponseSink::new(tx.into_sink());

    hub.invoke_text(request, Default::default(), sink)
        .await
        .unwrap();

    let response = rx.recv().unwrap().unwrap_text().strip_record_separator();
    let response: protocol::Completion<Vec<usize>> = serde_json::from_str(&response).unwrap();

    let expected_response =
        protocol::Completion::new("123".to_string(), Some(vec![0, 1, 2, 3, 4]), None);

    assert_eq!(expected_response, response);
}

#[tokio::test]
async fn stream_invocation() {
    async fn stream(Args(count): Args<usize>) -> impl HubResponse {
        HubStream::infallible(stream! {
            for i in 0..count {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                yield i;
            }
        })
    }

    let hub = HubBuilder::new().method("stream", stream).build();

    let invocation =
        protocol::StreamInvocation::new("123".to_string(), "stream".to_string(), Some((3usize,)));
    let request = serde_json::to_string(&invocation).unwrap();

    dbg!(request.clone());

    let (tx, rx) = flume::bounded(10);

    let sink = ResponseSink::new(tx.into_sink());

    hub.invoke_text(request, Default::default(), sink)
        .await
        .unwrap();

    let mut response = rx.into_stream();

    let f1 = response
        .next()
        .await
        .unwrap()
        .unwrap_text()
        .strip_record_separator();
    let f2 = response
        .next()
        .await
        .unwrap()
        .unwrap_text()
        .strip_record_separator();
    let f3 = response
        .next()
        .await
        .unwrap()
        .unwrap_text()
        .strip_record_separator();
    let completion = response
        .next()
        .await
        .unwrap()
        .unwrap_text()
        .strip_record_separator();
    let none = response.next().await;

    let expected_f1 = protocol::StreamItem::new("123".to_string(), 0usize);
    let expected_f2 = protocol::StreamItem::new("123".to_string(), 1usize);
    let expected_f3 = protocol::StreamItem::new("123".to_string(), 2usize);
    let expected_completion = protocol::Completion::<usize>::new("123".to_string(), None, None);

    let actual_f1 = serde_json::from_str(&f1).unwrap();
    let actual_f2 = serde_json::from_str(&f2).unwrap();
    let actual_f3 = serde_json::from_str(&f3).unwrap();
    let actual_completion = serde_json::from_str(&completion).unwrap();

    assert_eq!(expected_f1, actual_f1);
    assert_eq!(expected_f2, actual_f2);
    assert_eq!(expected_f3, actual_f3);
    assert_eq!(expected_completion, actual_completion);
    assert!(none.is_none());
}

#[tokio::test]
async fn stream_failure_invocation() {
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

    let invocation = protocol::StreamInvocation::new(
        "123".to_string(),
        "stream_failure".to_string(),
        Some((3usize,)),
    );

    let request = serde_json::to_string(&invocation).unwrap();

    let (tx, rx) = flume::bounded(10);

    let sink = ResponseSink::new(tx.into_sink());

    hub.invoke_text(request, Default::default(), sink)
        .await
        .unwrap();

    let mut response = rx.into_stream();
    let f1 = response
        .next()
        .await
        .unwrap()
        .unwrap_text()
        .strip_record_separator();
    let f2 = response
        .next()
        .await
        .unwrap()
        .unwrap_text()
        .strip_record_separator();
    let f3 = response
        .next()
        .await
        .unwrap()
        .unwrap_text()
        .strip_record_separator();
    let completion = response
        .next()
        .await
        .unwrap()
        .unwrap_text()
        .strip_record_separator();
    let none = response.next().await;

    let expected_f1 = protocol::StreamItem::new("123".to_string(), 0usize);
    let expected_f2 = protocol::StreamItem::new("123".to_string(), 1usize);
    let expected_f3 = protocol::StreamItem::new("123".to_string(), 2usize);
    let expected_completion = protocol::Completion::<usize>::new(
        "123".to_string(),
        None,
        Some("Ran out of data!".to_string()),
    );

    let actual_f1 = serde_json::from_str(&f1).unwrap();
    let actual_f2 = serde_json::from_str(&f2).unwrap();
    let actual_f3 = serde_json::from_str(&f3).unwrap();
    let actual_completion = serde_json::from_str(&completion).unwrap();

    assert_eq!(expected_f1, actual_f1);
    assert_eq!(expected_f2, actual_f2);
    assert_eq!(expected_f3, actual_f3);
    assert_eq!(expected_completion, actual_completion);
    assert!(none.is_none());
}

trait StringExt {
    fn strip_record_separator(self) -> String;
}

impl StringExt for String {
    fn strip_record_separator(self) -> String {
        self.trim_end_matches(WEIRD_ENDING).to_owned()
    }
}
