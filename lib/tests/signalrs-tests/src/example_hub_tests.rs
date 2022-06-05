#![allow(unused_imports)]
use futures::{
    sink::Sink,
    stream::{Stream, StreamExt},
};
use playground::example_hub;
use signalrs_core::protocol;

const WEIRD_ENDING: &str = "\u{001E}";

#[tokio::test]
async fn example_hub_simple_invocation_succesfull() {
    let hub = example_hub::HubInvoker::new();

    let invocation = protocol::Invocation::new(
        Some("123".to_string()),
        "add".to_string(),
        Some((1i32, 2i32)),
    );
    let request = serde_json::to_string(&invocation).unwrap();

    let (tx, rx) = flume::bounded(1);

    hub.invoke_text(request, tx.into_sink().clone())
        .await
        .unwrap();

    let response = rx.recv().unwrap().strip_record_separator();
    let response: protocol::Completion<i32> = serde_json::from_str(&response).unwrap();

    let expected_response = protocol::Completion::new("123".to_string(), Some(3), None);

    assert_eq!(expected_response, response);
}

#[tokio::test]
async fn example_hub_simple_invocation_failed() {
    let hub = example_hub::HubInvoker::new();

    let invocation = protocol::Invocation::new(
        Some("123".to_string()),
        "single_result_failure".to_string(),
        Some((1i32, 2i32)),
    );
    let request = serde_json::to_string(&invocation).unwrap();

    let (tx, rx) = flume::bounded(1);

    hub.invoke_text(request, tx.into_sink()).await.unwrap();

    let response = rx.recv().unwrap().strip_record_separator();
    let response: protocol::Completion<i32> = serde_json::from_str(&response).unwrap();

    let expected_response =
        protocol::Completion::new("123".to_string(), None, Some("An error!".to_string()));

    assert_eq!(expected_response, response);
}

#[tokio::test]
async fn example_hub_batched_invocation() {
    let hub = example_hub::HubInvoker::new();

    let invocation = protocol::Invocation::new(
        Some("123".to_string()),
        "batched".to_string(),
        Some((5usize, ())),
    );
    let request = serde_json::to_string(&invocation).unwrap();

    let (tx, rx) = flume::bounded(1);

    hub.invoke_text(request, tx.into_sink()).await.unwrap();

    let response = rx.recv().unwrap().strip_record_separator();
    let response: protocol::Completion<Vec<usize>> = serde_json::from_str(&response).unwrap();

    let expected_response =
        protocol::Completion::new("123".to_string(), Some(vec![0, 1, 2, 3, 4]), None);

    assert_eq!(expected_response, response);
}

#[tokio::test]
async fn example_hub_stream_invocation() {
    let hub = example_hub::HubInvoker::new();

    let invocation = protocol::StreamInvocation::new(
        "123".to_string(),
        "stream".to_string(),
        Some((3usize, ())),
    );
    let request = serde_json::to_string(&invocation).unwrap();

    let (tx, rx) = flume::bounded(10);

    hub.invoke_text(request, tx.into_sink()).await.unwrap();

    let mut response = rx.into_stream();

    let f1 = response.next().await.unwrap().strip_record_separator();
    let f2 = response.next().await.unwrap().strip_record_separator();
    let f3 = response.next().await.unwrap().strip_record_separator();
    let completion = response.next().await.unwrap().strip_record_separator();
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
async fn example_hub_stream_failure_invocation() {
    let hub = example_hub::HubInvoker::new();

    let invocation = protocol::StreamInvocation::new(
        "123".to_string(),
        "stream_failure".to_string(),
        Some((3usize, ())),
    );
    let request = serde_json::to_string(&invocation).unwrap();

    let (tx, rx) = flume::bounded(10);

    hub.invoke_text(request, tx.into_sink()).await.unwrap();

    let mut response = rx.into_stream();
    let f1 = response.next().await.unwrap().strip_record_separator();
    let f2 = response.next().await.unwrap().strip_record_separator();
    let f3 = response.next().await.unwrap().strip_record_separator();
    let completion = response.next().await.unwrap().strip_record_separator();
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
