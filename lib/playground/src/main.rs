// use playground::{example_hub, ws_transport};
use async_stream::stream;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Extension,
    },
    http::{Method, StatusCode},
    response::{IntoResponse, Json},
    routing::{get, get_service, post},
    Router,
};
use futures::{select, sink::SinkExt, stream::StreamExt, FutureExt, Stream};
use log::*;
use signalrs::{
    connection::ConnectionState,
    extract::{Args, UploadStream},
    hub::{builder::HubBuilder, Hub},
    negotiate::{NegotiateResponseV0, TransportSpec},
    response::ResponseSink,
};
use simple_logger::SimpleLogger;
use std::sync::Arc;
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    services::fs::ServeFile,
    trace::TraceLayer,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // tracing_subscriber::fmt::init();

    SimpleLogger::new()
        .with_colors(true)
        .with_local_timestamps()
        .with_module_level("mio", LevelFilter::Warn)
        .with_module_level("tokio_tungstenite", LevelFilter::Warn)
        .with_module_level("tungstenite", LevelFilter::Warn)
        .with_module_level("hyper", LevelFilter::Warn)
        .with_module_level("tracing", LevelFilter::Warn)
        .with_module_level("tower_http", LevelFilter::Warn)
        .init()
        .unwrap();

    let cors = CorsLayer::new()
        .allow_headers(vec![
            "x-requested-with".parse()?,
            "x-signalr-user-agent".parse()?,
        ])
        .allow_credentials(true)
        .allow_methods(vec![Method::POST])
        .allow_origin(AllowOrigin::exact("http://localhost:8080".parse()?));

    let index = get_service(ServeFile::new(
        "/home/radoslaw/Programowanie/signalrs/web/index.html",
    ))
    .handle_error(|e| async move {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Unhandled exception: {}", e),
        )
    });

    let hub_builder = HubBuilder::new()
        .method("add", add)
        .method("add_stream", add_stream)
        .streaming_method("stream", stream)
        .streaming_method("stream2", stream2);

    let invoker = Arc::new(hub_builder.build());

    let app = Router::new()
        .route("/", index)
        .route("/chathub/negotiate", post(negotiate).layer(cors))
        .route(
            "/chathub",
            get(ws_upgrade_handler).layer(Extension(invoker)),
        )
        .layer(TraceLayer::new_for_http());

    axum::Server::bind(&"0.0.0.0:8080".parse()?)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

async fn negotiate() -> Json<NegotiateResponseV0> {
    debug!("negotiate endpoint invoked");

    Json(NegotiateResponseV0 {
        connection_id: "test".into(),
        negotiate_version: 0,
        available_transports: vec![TransportSpec {
            transport: "WebSockets".into(),
            transfer_formats: vec!["Text".into()],
        }],
    })
}

async fn ws_upgrade_handler(
    ws: WebSocketUpgrade,
    Extension(invoker): Extension<Arc<Hub>>,
) -> impl IntoResponse {
    debug!("websocket upgrade request recieved");

    let f = |ws| ws_handler(ws, invoker);
    ws.on_upgrade(f)
}

async fn ws_handler(socket: WebSocket, invoker: Arc<Hub>) {
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
                            let invoker = Arc::clone(&invoker);
                            let itx = tx_channel.clone();
                            let csc = connection_state.clone();
                            tokio::spawn(async move {
                                if let Err(e) = invoker.invoke_text(f, csc, itx).await {
                                    error!("error on invoke_text: {}", e);
                                };
                            });
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

async fn add(Args((a, b)): Args<(i32, i32)>) -> i32 {
    a + b
}

pub async fn stream(Args(count): Args<usize>) -> impl Stream<Item = usize> {
    stream! {
        for i in 0..count {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            yield i;
        }
    }
}

pub async fn add_stream(mut input: UploadStream<i32>) -> i32 {
    let mut result = Vec::new();
    while let Some(i) = input.next().await {
        result.push(i);
    }

    result.into_iter().sum::<i32>()
}

pub async fn stream2(
    Args(count): Args<usize>,
    _input: UploadStream<i32>,
) -> impl Stream<Item = usize> {
    stream! {
        for i in 0..count {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            yield i;
        }
    }
}
