use crate::{
    error,
    protocol::{HandshakeRequest, HandshakeResponse},
    Command,
};
use crate::{messages::ClientMessage, TransportClientHandle};
use futures::{select, FutureExt, SinkExt, StreamExt, Stream};
use std::{
    fmt::Display,
    time::{Duration, Instant},
};
use thiserror::Error;
use tokio::net::TcpStream;
use tokio_tungstenite::{tungstenite::Message, MaybeTlsStream, WebSocketStream};
use tracing::*;

use super::error::TransportError;

pub(crate) struct WebSocketTransport { 
    websocket: WebSocketStream<MaybeTlsStream<TcpStream>>,
    close_requested: bool
}

impl WebSocketTransport {
    pub async fn connect(url: impl ToString) -> Result<WebSocketTransport, ConnectError> {
        let (websocket, _) = tokio_tungstenite::connect_async(url.to_string()).await?;
        
        Ok(WebSocketTransport {
            websocket,
            close_requested: false
        })
    }

    pub async fn signalr_handshake(&mut self) -> Result<(), HandshakeError> {
        self.websocket
            .send(Message::Text(HandshakeRequest::new("json").try_into()?))
            .await?;
    
        let response = match self.websocket.next().await {
            Some(response) => response?,
            None => return Err(HandshakeError::StreamEnded),
        };
    
        match response {
            Message::Text(value) => {
                if let HandshakeResponse { error: Some(error) } = value.try_into()? {
                    Err(HandshakeError::Protocol(error))
                } else {
                    Ok(())
                }
            }
            _ => {
                Err(HandshakeError::Protocol(
                    "incomprehensible handshake response".to_owned(),
                ))
            }
        }
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub(crate) enum ConnectError {
    #[error("WebSocket error")]
    Websocket(#[from] tokio_tungstenite::tungstenite::Error),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub(crate) enum HandshakeError {
    #[error("handshake payload invalid")]
    InvalidPayload(#[from] serde_json::Error),
    #[error("WebSocket error")]
    Websocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("{0}")]
    Protocol(String),
    #[error("connection lost")]
    StreamEnded,
}


impl WebSocketTransport {
    pub async fn send_text(&mut self, message: impl Into<String>) -> Result<(), WebSocketSendTextError>
    {
        self.websocket
            .send(Message::Text(message.into()))
            .await
            .map_err(|e| e.into())
    }
}

#[derive(Debug, Error)]
pub(crate) enum WebSocketSendTextError {
    #[error("WebSocket error")]
    Websocket(#[from] tokio_tungstenite::tungstenite::Error),
}


impl WebSocketTransport {
    pub async fn receive(&mut self) -> Result<Option<String>, WebSocketReceiveError>
    {
        let message = match self.websocket.next().await {
            Some(message) => message?,
            None => return Err(WebSocketReceiveError::StreamEnded)
        };

        match message {
            Message::Text(text) => return Ok(Some(text)),
            Message::Binary(_) => return Err(WebSocketReceiveError::UnsupportedBinaryProtocol),
            Message::Ping(data) => self.send_pong(data).await?,
            Message::Close(_) => self.close_requested(),
            Message::Pong(_) | Message::Frame(_) => { /* ignore */ },
        };

        Ok(None)
    }

    async fn send_pong(&mut self, data: Vec<u8>) -> Result<(), tokio_tungstenite::tungstenite::Error> {
        self.websocket
            .send(Message::Pong(data))
            .await
    }

    fn close_requested(&mut self) {
        self.close_requested = true;
    }
}

#[derive(Debug, Error)]
pub(crate) enum WebSocketReceiveError {
    #[error("WebSocket error")]
    Websocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("WebSocket stream ended")]
    StreamEnded,
    #[error("received unsupported binary message")]
    UnsupportedBinaryProtocol
}

enum Event {
    Close,
    None,
}

pub(crate) async fn websocket_hub<'a>(
    mut websocket: WebSocketStream<MaybeTlsStream<TcpStream>>,
    client: TransportClientHandle,
    messages_to_send: flume::Receiver<ClientMessage>,
) {
    let mut last_life_sign = Instant::now();
    let mut messages_to_send = messages_to_send.into_stream().fuse();
    let mut health_check = tokio::time::interval(Duration::from_secs(30));
    
    loop {
        let span = debug_span!("websocket");
        select! {
            _ = health_check.tick().fuse() => send_ping(&mut websocket).await,
            message = messages_to_send.next() => {
                if message.is_none() {
                    break;
                }

                send_message(&mut websocket, message.unwrap()).instrument(span).await;
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
            event!(
                Level::ERROR,
                "websocker closing due to lack of life signs from server"
            );
            break;
        }
    }

    async fn send_ping(websocket: &mut WebSocketStream<MaybeTlsStream<TcpStream>>) {
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
            }
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
