use super::ClientMessage;
use futures::{select, SinkExt, Stream, StreamExt};
use log::error;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    tungstenite::{self, Message},
    MaybeTlsStream, WebSocketStream,
};

async fn websocket_hub<F1: Fn(ClientMessage), F2: Stream<Item = ClientMessage> + Unpin>(
    mut websocket: WebSocketStream<MaybeTlsStream<TcpStream>>,
    mut message_received_callback: F1,
    messages_to_send: F2,
) {
    let mut messages_to_send = messages_to_send.fuse();
    loop {
        select! {
            to_send = messages_to_send.next() => {
                if to_send.is_none() {
                    break;
                }

                send_message(&mut websocket, to_send.unwrap()).await;
            },
            received = websocket.next() => {
                if received.is_none() {
                    break;
                }

                match received.unwrap() {
                    Ok(message) => {
                        incoming_message(&mut websocket, message, &mut message_received_callback).await
                    }
                    Err(error) => incoming_message_error(error),
                }
            }
        }
    }

    async fn send_message(
        websocket: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
        message: ClientMessage,
    ) {
        let result = match message {
            ClientMessage::Json(text) => websocket.send(Message::Text(text)).await,
            ClientMessage::Binary(bytes) => websocket.send(Message::Binary(bytes)).await,
        };

        if let Err(error) = result {
            error!("{}", error);
        }
    }

    async fn incoming_message<F1: Fn(ClientMessage)>(
        websocket: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
        message: Message,
        message_received_callback: &mut F1,
    ) {
        match message {
            Message::Text(text) => message_received_callback(ClientMessage::Json(text)),
            Message::Binary(bytes) => message_received_callback(ClientMessage::Binary(bytes)),
            Message::Ping(payload) => send_pong(websocket, payload).await,
            Message::Pong(_) => { /* ignore for now, need to track time */ }
            Message::Close(_) => { /* probably need to send something, ignore for now */ }
            Message::Frame(_) => { /* apparently impossible to get while reading */ }
        }

        async fn send_pong(
            websocket: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
            ping_payload: Vec<u8>,
        ) {
            if let Err(error) = websocket.send(Message::Pong(ping_payload)).await {
                error!("{}", error)
            }
        }
    }

    fn incoming_message_error(error: tungstenite::Error) {
        error!("{}", error)
    }
}
