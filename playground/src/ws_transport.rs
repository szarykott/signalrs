use crate::example_hub::*;
use futures::{sink::SinkExt, stream::StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

pub async fn run(invoker: HubInvoker) -> Result<(), Box<dyn std::error::Error>> {
    let socket = TcpListener::bind("127.0.0.1:8080").await?;

    while let Ok((tcp_stream, _addr)) = socket.accept().await {
        let (mut tx, mut rx) = tokio_tungstenite::accept_async(tcp_stream).await?.split();

        while let Some(Ok(msg)) = rx.next().await {
            match msg {
                Message::Text(f) => {
                    dbg!(f.clone());
                    match invoker.invoke_text(&f).await {
                        HubResponse::Void => { /* skip */ }
                        HubResponse::Single(response) => {
                            tx.send(Message::Text(response)).await.unwrap()
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
            }
        }
    }

    Ok(())
}
