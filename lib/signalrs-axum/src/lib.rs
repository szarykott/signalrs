use axum::{
    body::Body,
    extract::ws::*,
    response::IntoResponse,
    routing::{get, post},
    Extension, Json, Router,
};
use futures::{future::FutureExt, select, sink::SinkExt, stream::StreamExt};
use log::*;
use signalrs::{
    connection::ConnectionState,
    hub::Hub,
    negotiate::NegotiateResponseV0,
    server::{response::ResponseSink, Server},
};
use std::sync::Arc;

pub fn hub_routes(hub: Hub) -> Router<Body> {
    let hub = Arc::new(hub);

    let router = Router::new()
        .route("/negotiate", post(negotiate))
        .route("/", get(ws_upgrade_handler).layer(Extension(hub)));

    router
}

async fn negotiate() -> Json<NegotiateResponseV0> {
    debug!("negotiate endpoint invoked");

    Json(NegotiateResponseV0::supported_spec(uuid::Uuid::new_v4()))
}

async fn ws_upgrade_handler(
    ws: WebSocketUpgrade,
    Extension(invoker): Extension<Arc<Server>>,
) -> impl IntoResponse {
    debug!("websocket upgrade request recieved");

    let f = |ws| ws_handler(ws, invoker);
    ws.on_upgrade(f)
}

async fn ws_handler(socket: WebSocket, invoker: Arc<Server>) {
    let (mut tx_socket, mut rx_socket) = socket.split();

    debug!("performing handshake for a new hub client");

    if let Some(Ok(Message::Text(msg))) = rx_socket.next().await {
        let response = invoker.handshake(msg.as_str());
        tx_socket.send(Message::Text(response)).await.unwrap(); // TODO: no unwrap!
    } else {
        //TODO: Proper error logging
        debug!("failed handshake attempt");
        return;
    }

    let (tx_channel, irx) = flume::bounded(1000);

    let tx_channel = ResponseSink::new(tx_channel.into_sink());
    let mut rx_channel = irx.into_stream(); // TODO: verify memory leak bug fixed!

    let connection_state: ConnectionState = Default::default();

    debug!("starting event loop for a new client");

    loop {
        select! {
            si = rx_channel.next() => match si {
                Some(msg) => {
                    let text = msg.unwrap_text();
                    debug!("sending to client: {}", text);
                    tx_socket.send(Message::Text(text)).await.unwrap();
                },
                None => {
                    debug!("rx_channel ended")
                    /* panik or kalm */
                },
            },
            nm = rx_socket.next().fuse() => match nm {
                Some(Ok(msg)) => {
                    debug!("received from client: {:?}", msg);
                    match msg {
                        Message::Text(f) => {
                            if let Err(e) = invoker.invoke_text(f, connection_state.clone(), tx_channel.clone()).await {
                                error!("error on invoke_text: {}", e);
                            };
                        }
                        Message::Binary(_) => {
                            // do nothing
                        }
                        Message::Ping(d) => tx_socket.send(Message::Pong(d)).await.unwrap(),
                        Message::Pong(_) => { /* ignore */ }
                        Message::Close(_) => { /* ignore */ }
                    };
                }
                Some(Err(e)) => {
                    debug!("error while receiving from client: {}", e);
                }
                None => {
                    break;
                }
            }
        }
    }

    debug!("client event loop stopped");
}
