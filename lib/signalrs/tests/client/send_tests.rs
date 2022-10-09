use super::super::common::{ClientOutputWrapper, ReceiverExt};
use flume::{r#async::RecvStream, Receiver};
use serde_json::json;
use signalrs::client::{self, ClientMessage, SignalRClient};

#[tokio::test]
async fn test_send0() {
    let (mut client, rx) = build_client();

    client.send0("fun").await.unwrap();

    let expected = json!({
        "type" : 1,
        "target": "fun"
    });

    let actual = rx.next_json_value();

    assert_eq!(expected, actual);
}

#[tokio::test]
async fn test_send1() {
    let (mut client, rx) = build_client();

    client.send1("fun", 1).await.unwrap();

    let expected = json!({
        "type" : 1,
        "target": "fun",
        "arguments": [1]
    });

    let actual = rx.next_json_value();

    assert_eq!(expected, actual);
}

#[tokio::test]
async fn test_send2() {
    let (mut client, rx) = build_client();

    client.send2("fun", 1, "a").await.unwrap();

    let expected = json!({
        "type" : 1,
        "target": "fun",
        "arguments": [1, "a"]
    });

    let actual = rx.next_json_value();

    assert_eq!(expected, actual);
}

// ============== HELPERS ======================== //

fn build_client() -> (
    SignalRClient<ClientOutputWrapper<ClientMessage>, RecvStream<'static, ClientMessage>>,
    Receiver<ClientMessage>,
) {
    let (client_output_sender, client_output_receiver) = flume::bounded::<ClientMessage>(100);
    let (_, client_input_receiver) = flume::bounded::<ClientMessage>(100);

    let client_output_sender =
        ClientOutputWrapper::<ClientMessage>::new_text(client_output_sender.into_sink());

    let client = client::new_text_client(client_output_sender, client_input_receiver.into_stream());

    (client, client_output_receiver)
}
