use crate::{
    caller::{Command, error::ClientError},
    messages,
    protocol::{HandshakeRequest, HandshakeResponse},
};
use crate::{caller::TransportClientHandle, messages::ClientMessage};
use futures::{select, SinkExt, StreamExt, FutureExt};
use std::{fmt::Display, time::{Duration, Instant}};
use tokio::net::TcpStream;
use tokio_tungstenite::{tungstenite::Message, MaybeTlsStream, WebSocketStream};
use tracing::*;
use super::TransportError;

enum Event {
    Close,
    None
}

pub(crate) async fn handshake(
    websocket: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
) -> Result<(), TransportError> {
    let request = messages::to_json(&HandshakeRequest::new("json"))?;
    websocket.send(Message::Text(request)).await?;
    let response = websocket
        .next()
        .await
        .ok_or_else(|| TransportError::BadReceive)??;

    match response {
        Message::Text(value) => {
            let stripped = messages::strip_record_separator(&value);
            let response: HandshakeResponse = serde_json::from_str(stripped)?;
            if response.is_error() {
                return Err(TransportError::from(ClientError::handshake(response.unwrap_error())));
            }
        }
        _ => { 
            return Err(TransportError::from(ClientError::handshake("incomprehensible handshake response")));
        }
    }

    Ok(())
}

pub(crate) async fn websocket_hub<'a>(
    mut websocket: WebSocketStream<MaybeTlsStream<TcpStream>>,
    client: TransportClientHandle,
    messages_to_send: flume::Receiver<ClientMessage>,
) {
    let mut last_life_sign = Instant::now();
    let mut messages_to_send = messages_to_send.into_stream().fuse();
    let mut ticks = tokio::time::interval(Duration::from_secs(30));
    loop {
        let span = debug_span!("websocket");
        select! {
            _ = ticks.tick().fuse() => {
                send_ping(&mut websocket).await;
            },
            to_send = messages_to_send.next() => {
                if to_send.is_none() {
                    break;
                }

                send_message(&mut websocket, to_send.unwrap()).instrument(span).await;
            },
            received = websocket.next() => {
                if received.is_none() {
                    break;
                }

                last_life_sign = Instant::now();

                match received.unwrap() {
                    Ok(message) => {
                        match incoming_message(&mut websocket, message, &client).instrument(span).await {
                            Ok(Event::None) => {  },
                            Ok(Event::Close) => break,
                            Err(error) => incoming_message_error(error)
                        };
                    }
                    Err(error) => incoming_message_error(error),
                }
            }
        }

        if Instant::now() - last_life_sign > Duration::from_secs(60) {
            event!(Level::ERROR, "websocker closing due to lack of life signs from server");
            break;
        }
    }
    
    async fn send_ping(
        websocket: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
    ) {
        if let Err(error) = websocket.send(Message::Ping(Vec::new())).await {
            error!("{}", error)
        }
    }

    async fn send_message(
        websocket: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
        message: ClientMessage,
    ) {
        let result = match message {
            ClientMessage::Json(text) => {
                event!(Level::TRACE, text, "text message sent",);
                websocket.send(Message::Text(text)).await
            }
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
    ) -> Result<Event, TransportError> {
        return match message {
            Message::Text(text) => {
                event!(Level::TRACE, text, "text message received");
                let command = client.receive_messages(ClientMessage::Json(text))?;
                Ok(command.into())
            }
            Message::Binary(bytes) => {
                event!(Level::TRACE, length = bytes.len(), "binary message received");
                let command = client.receive_messages(ClientMessage::Binary(bytes))?;
                Ok(command.into())
            },
            Message::Ping(payload) => {
                send_pong(websocket, payload).await;
                return Ok(Event::None);
            }
            Message::Pong(_) => {
                return Ok(Event::None);
            }
            Message::Close(_) => {
                return Ok(Event::Close);
            }
            Message::Frame(_) => {
                /* apparently impossible to get while reading */
                return Ok(Event::None);
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
        error!("error during reception of message: {}", error)
    }
}

impl From<Command> for Event {
    fn from(command: Command) -> Self {
        match command {
            Command::None => Event::None,
            Command::Close => Event::Close,
        }
    }
}