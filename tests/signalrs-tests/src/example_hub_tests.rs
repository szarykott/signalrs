use playground::example_hub;
use serde_json;
use signalrs_core::protocol;
use tokio;
use futures::stream::StreamExt;

#[tokio::test]
async fn example_hub_simple_invocation_succesfull() {
    let hub = example_hub::HubInvoker::new();

    let invocation =
        protocol::Invocation::new(Some("123".to_string()), "add".to_string(), Some((1, 2)));
    let request = serde_json::to_string(&invocation).unwrap();

    let response = hub.invoke_text(&request).await;

    dbg!(response.clone());

    let response = response.unwrap_single();
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
        Some((1, 2)),
    );
    let request = serde_json::to_string(&invocation).unwrap();

    let response = hub.invoke_text(&request).await;

    dbg!(response.clone());

    let response = response.unwrap_single();
    let response: protocol::Completion<i32> = serde_json::from_str(&response).unwrap();

    let expected_response =
        protocol::Completion::new("123".to_string(), None, Some("An error!".to_string()));

    assert_eq!(expected_response, response);
}

#[tokio::test]
async fn example_hub_batched_invocation() {
    let hub = example_hub::HubInvoker::new();

    let invocation =
        protocol::Invocation::new(Some("123".to_string()), "batched".to_string(), Some((5usize, ())));
    let request = serde_json::to_string(&invocation).unwrap();

    let response = hub.invoke_text(&request).await;

    dbg!(response.clone());

    let response = response.unwrap_single();
    let response: protocol::Completion<Vec<usize>> = serde_json::from_str(&response).unwrap();

    let expected_response =
        protocol::Completion::new("123".to_string(), Some(vec![0, 1, 2, 3, 4]), None);

    assert_eq!(expected_response, response);
}

#[tokio::test]
async fn example_hub_stream_invocation() {
    let hub = example_hub::HubInvoker::new();

    let invocation =
        protocol::Invocation::new(Some("123".to_string()), "stream".to_string(), Some((3usize, ())));
    let request = serde_json::to_string(&invocation).unwrap();

    let response = hub.invoke_text(&request).await;

    let mut response = response.unwrap_stream();
    let f1 = response.next().await.unwrap();
    let f2 = response.next().await.unwrap();
    let f3 = response.next().await.unwrap();
    let completion = response.next().await.unwrap();
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