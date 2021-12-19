use crate::example_hub;
use futures::{sink::SinkExt, stream::StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

pub async fn run(invoker: example_hub::HubInvoker) -> Result<(), Box<dyn std::error::Error>> {
    let socket = TcpListener::bind("127.0.0.1:8080").await?;

    while let Ok((tcp_stream, _addr)) = socket.accept().await {
        let (mut outgoing, mut incoming) =
            tokio_tungstenite::accept_async(tcp_stream).await?.split();

        while let Some(Ok(msg)) = incoming.next().await {
            match msg {
                Message::Text(f) => {
                    dbg!(f.clone());
                    let response = invoker.invoke_text(&f).await;
                    outgoing.send(Message::Text(response)).await.unwrap();
                }
                Message::Binary(f) => {
                    let response = invoker.invoke_binary(&f).await;
                    outgoing.send(Message::Binary(response)).await.unwrap();
                }
                Message::Ping(d) => outgoing.send(Message::Pong(d)).await.unwrap(),
                Message::Pong(_) => { /* ignore */ }
                Message::Close(_) => { /* ignore */ }
            }
        }
    }

    Ok(())
}
