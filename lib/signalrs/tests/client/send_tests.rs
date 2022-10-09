use super::super::common::{ClientOutputWrapper, ReceiverExt};
use flume::{r#async::RecvStream, Receiver};
use serde_json::json;
use signalrs::client::{self, ClientMessage, SignalRClient};

macro_rules! test_send {
    ($name:ident, $fn:ident, $($arg:literal),*) => {
        #[tokio::test]
        async fn $name() {
            let (mut client, rx) = build_client();

            client.$fn("fun", $($arg,)*).await.unwrap();

            let expected = json!({
                "type" : 1,
                "target": "fun",
                "arguments" : [$($arg,)*]
            });

            let actual = rx.next_json_value();

            assert_eq!(expected, actual);
        }
    };
}
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

test_send!(test_send1, send1, 1);
test_send!(test_send2, send2, 1, "a");
test_send!(test_send3, send3, 1, 2, 3);
test_send!(test_send4, send4, 1, 2, 3, true);
test_send!(test_send5, send5, 1, 2, 3usize, true, "c");
test_send!(test_send6, send6, 1, 2, 3usize, true, "c", 1.1);
test_send!(test_send7, send7, 1, 2, 3usize, true, "c", 1.2, "ameba");
test_send!(test_send8, send8, 1, 2, 3usize, true, "c", 1.2, "ameba", "wololoo");
test_send!(test_send9, send9, 1, 2, 3usize, true, "c", 1.2, "ameba", "wololoo", false);
test_send!(
    test_send10,
    send10,
    1,
    2,
    3usize,
    true,
    "c",
    1.2,
    "ameba",
    "wololoo",
    false,
    -1
);

test_send!(
    test_send11,
    send11,
    1,
    2,
    3usize,
    true,
    "c",
    1.2,
    "ameba",
    "wololoo",
    false,
    -1,
    "dwa"
);

test_send!(
    test_send12,
    send12,
    1,
    2,
    3usize,
    true,
    "c",
    1.2,
    "ameba",
    "wololoo",
    false,
    -1,
    "dwa",
    "expert"
);

test_send!(
    test_send13,
    send13,
    1,
    2,
    3usize,
    true,
    "c",
    1.2,
    "ameba",
    "wololoo",
    false,
    -1,
    "dwa",
    "expert",
    1
);

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
