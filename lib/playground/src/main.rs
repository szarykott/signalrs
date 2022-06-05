// use playground::{example_hub, ws_transport};
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
use futures::{select, sink::SinkExt, stream::StreamExt, FutureExt};
use playground::example_hub::HubInvoker;
use signalrs_core::negotiate::{NegotiateResponseV0, TransportSpec};
use std::sync::Arc;
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    services::fs::ServeFile,
    trace::TraceLayer,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

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

    let invoker = Arc::new(HubInvoker::new());

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
    Extension(invoker): Extension<Arc<HubInvoker>>,
) -> impl IntoResponse {
    let f = |ws| ws_handler(ws, invoker);
    ws.on_upgrade(f)
}

async fn ws_handler(socket: WebSocket, invoker: Arc<HubInvoker>) {
    let (mut tx, mut rx) = socket.split();

    if let Some(Ok(Message::Text(msg))) = rx.next().await {
        let response = invoker.handshake(msg.as_str());
        tx.send(Message::Text(response)).await.unwrap();
    } else {
        return;
    }

    let (itx, irx) = flume::bounded(1000);
    let itx = itx.into_sink();
    let mut irx = irx.into_stream(); // TODO: verify memory leak bug fixed!

    loop {
        select! {
            si = irx.next() => match si {
                Some(msg) => {
                    let msg : String = msg;
                    dbg!(msg.clone());
                    tx.send(Message::Text(msg)).await.unwrap();
                },
                None => { /* panik */ },
            },
            nm = rx.next().fuse() => match nm {
                Some(Ok(msg)) => {
                    dbg!(msg.clone());
                    match msg {
                        Message::Text(f) => {
                            let invoker = Arc::clone(&invoker);
                            let itx = itx.clone();
                            tokio::spawn(async move {
                                if let Err(_e) = invoker.invoke_text(f, itx).await {
                                    // TODO: log e?
                                };
                            });
                        }
                        Message::Binary(f) => {
                            let response = invoker.invoke_binary(&f).await;
                            dbg!(response.clone());
                            tx.send(Message::Binary(response)).await.unwrap();
                        }
                        Message::Ping(d) => tx.send(Message::Pong(d)).await.unwrap(),
                        Message::Pong(_) => { /* ignore */ }
                        Message::Close(_) => { /* ignore */ }
                    };
                }
                Some(Err(e)) => {
                    dbg!(e);
                }
                None => {
                    dbg!("None");
                    break;
                }
            }
        }
    }
}
