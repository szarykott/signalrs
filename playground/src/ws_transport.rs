use futures::{
    stream::{SplitSink, SplitStream, Stream, StreamExt},
    task::Poll,
    sink::{Sink, SinkExt}
};
use std::{collections::HashMap, hash::Hash, net::SocketAddr};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};

pub async fn run(
    mut hub_sink: Box<dyn Sink<Vec<u8>, Error = Box<dyn std::error::Error>> + Unpin>,
    mut hub_stream: Box<dyn Stream<Item = Vec<u8>> + Unpin>
) -> Result<(), Box<dyn std::error::Error>> {
    let socket = TcpListener::bind("127.0.0.1:8080").await?;

    if let Ok((tcp_stream, addr)) = socket.accept().await {
        let (mut outgoing, mut incoming) = tokio_tungstenite::accept_async(tcp_stream).await?.split();


        loop {
            if let Some(Ok(msg)) = incoming.next().await {
                match msg {
                    Message::Text(f) => {
                        hub_sink.send(f.into_bytes()).await.unwrap()
                    },
                    Message::Binary(f) => hub_sink.send(f).await.unwrap(),
                    Message::Ping(d) => outgoing.send(Message::Pong(d)).await.unwrap(),
                    Message::Pong(_) => { /* ignore */ },
                    Message::Close(_) => { /* ignore */ },
                }
            }

            if let Some(msg) = hub_stream.next().await {
                outgoing.send(Message::Binary(msg)).await.unwrap();
            }
        }

    }

    Ok(())
}
