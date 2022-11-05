use std::fmt::Display;

use crate::client::{client2::Command, SignalRClientError};

use super::{
    client2::{TransportClientHandle},
    ClientMessage,
};
use futures::{select, SinkExt, StreamExt};
use log::error;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    tungstenite::{Message},
    MaybeTlsStream, WebSocketStream,
};

pub(crate) async fn websocket_hub<'a>(
    mut websocket: WebSocketStream<MaybeTlsStream<TcpStream>>,
    client: TransportClientHandle,
    messages_to_send: flume::Receiver<ClientMessage>,
) {
    let mut messages_to_send = messages_to_send.into_stream().fuse();
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
                        match incoming_message(&mut websocket, message, &client).await {
                            Ok(Command::Close) => break,
                            Ok(Command::None) => {  },
                            Err(error) => incoming_message_error(error)
                        };
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

    async fn incoming_message<'a>(
        websocket: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
        message: Message,
        client: &'a TransportClientHandle,
    ) -> Result<Command, SignalRClientError> {
        return match message {
            Message::Text(text) => client.receive_message(ClientMessage::Json(text)),
            Message::Binary(bytes) => client.receive_message(ClientMessage::Binary(bytes)),
            Message::Ping(payload) => {
                send_pong(websocket, payload).await;
                return Ok(Command::None);
            }
            Message::Pong(_) => {
                /* ignore for now, need to track time */
                return Ok(Command::None);
            }
            Message::Close(_) => {
                /* probably need to send something, ignore for now */
                return Ok(Command::Close);
            }
            Message::Frame(_) => {
                /* apparently impossible to get while reading */
                return Ok(Command::None);
            }
        };

        async fn send_pong(
            websocket: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
            ping_payload: Vec<u8>,
        ) {
            if let Err(error) = websocket.send(Message::Pong(ping_payload)).await {
                error!("{}", error)
            }
        }
    }

    fn incoming_message_error(error: impl Display) {
        error!("{}", error)
    }
}
