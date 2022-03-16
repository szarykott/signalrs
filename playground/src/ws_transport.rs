use std::{vec, error::Error};

use crate::example_hub::*;
use futures::{sink::SinkExt, stream::StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;

pub async fn run(mut invoker: HubInvoker) -> Result<(), Box<dyn std::error::Error>> {
    let socket = TcpListener::bind("127.0.0.1:8080").await?;

    while let Ok((mut tcp_stream, _addr)) = socket.accept().await {
        let _result = match is_negotiate(&mut tcp_stream).await {
            Ok(true) => handle_negotiate(&mut tcp_stream).await,
            Ok(false) => handle_websocket(tcp_stream, &mut invoker).await,
            Err(_e) => todo!()
        };
    }

    Ok(())
}

async fn is_negotiate(tcp_stream: &mut TcpStream) -> Result<bool, Box<dyn Error>> {
    tcp_stream.readable().await?;

    let mut buffer = vec![0u8; 256];

    tcp_stream.peek(&mut buffer).await?;    

    let line = String::from_utf8(buffer)?;

    let result = line.contains("negotiate?negotiateVersion=");

    Ok(result)
}

async fn handle_negotiate(tcp_stream: &mut TcpStream) -> Result<(), Box<dyn Error>> {
    println!("Handling negotiate");
    Ok(())
}

async fn handle_websocket(tcp_stream: TcpStream, invoker: &mut HubInvoker) -> Result<(), Box<dyn Error>> {
    let (mut tx, mut rx) = tokio_tungstenite::accept_async(tcp_stream).await?.split();

    while let Some(Ok(msg)) = rx.next().await {
        match msg {
            Message::Text(f) => {
                dbg!(f.clone());
                match invoker.invoke_text(&f).await {
                    HubResponse::Void => { /* skip */ }
                    HubResponse::Single(response) => {
                        tx.send(Message::Text(response)).await?;
                    }
                    HubResponse::Stream(_) => todo!(),
                }
            }
            Message::Binary(f) => {
                let response = invoker.invoke_binary(&f).await;
                tx.send(Message::Binary(response)).await.unwrap();
            }
            Message::Ping(d) => tx.send(Message::Pong(d)).await.unwrap(),
            Message::Pong(_) => { /* ignore */ }
            Message::Close(_) => { /* ignore */ }
        };
    }

    Ok(())
}
