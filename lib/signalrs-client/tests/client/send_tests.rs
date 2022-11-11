




// #[tokio::test]
// async fn test_send_no_args() -> Result<(), Box<dyn std::error::Error>> {
//     let (mut client, rx) = build_client();

//     client.method("fun").send().await?;

//     let expected = json!({
//         "type" : 1,
//         "target": "fun"
//     });

//     let actual = rx.next_json_value();

//     assert_eq!(expected, actual);

//     Ok(())
// }

// // ============== HELPERS ======================== //

// fn build_client() -> (
//     SignalRClient<ClientOutputWrapper<ClientMessage>, RecvStream<'static, ClientMessage>>,
//     Receiver<ClientMessage>,
// ) {
//     let (client_output_sender, client_output_receiver) = flume::bounded::<ClientMessage>(100);
//     let (_, client_input_receiver) = flume::bounded::<ClientMessage>(100);

//     let client_output_sender =
//         ClientOutputWrapper::<ClientMessage>::new_text(client_output_sender.into_sink());

//     let client =
//         client::new_text_client(client_output_sender, client_input_receiver.into_stream(), None);

//     (client, client_output_receiver)
// }
