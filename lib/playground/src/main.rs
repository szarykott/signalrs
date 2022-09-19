// use playground::{example_hub, ws_transport};
use async_stream::stream;
use axum::{body::Body, http::StatusCode, routing::get_service, Router};
use futures::{stream::StreamExt, Stream};
use log::*;
use signalrs::{extract::UploadStream, hub::builder::HubBuilder};
use signalrs_axum::hub_routes;
use simple_logger::SimpleLogger;
use tower_http::{cors::CorsLayer, services::fs::ServeFile, trace::TraceLayer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    let index = get_service(ServeFile::new(
        "/home/radoslaw/Programowanie/signalrs/web/index.html",
    ))
    .handle_error(|e| async move {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Unhandled exception: {}", e),
        )
    });

    let app = Router::new()
        .route("/", index)
        .nest("/chathub", build_hub())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    axum::Server::bind(&"0.0.0.0:8080".parse()?)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

fn build_hub() -> Router<Body> {
    let hub_builder = HubBuilder::new()
        .method("add", add)
        .method("add_stream", add_stream)
        .streaming_method("stream", stream)
        .streaming_method("stream2", stream2);

    hub_routes(hub_builder.build())
}

async fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub async fn stream(count: usize) -> impl Stream<Item = usize> {
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

pub async fn stream2(count: usize, _input: UploadStream<i32>) -> impl Stream<Item = usize> {
    stream! {
        for i in 0..count {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            yield i;
        }
    }
}

fn setup_logger() {
    // tracing_subscriber::fmt::init();

    SimpleLogger::new()
        .with_colors(true)
        .with_module_level("mio", LevelFilter::Warn)
        .with_module_level("tokio_tungstenite", LevelFilter::Warn)
        .with_module_level("tungstenite", LevelFilter::Warn)
        .with_module_level("hyper", LevelFilter::Warn)
        .with_module_level("tracing", LevelFilter::Warn)
        .with_module_level("tower_http", LevelFilter::Warn)
        .with_utc_timestamps()
        .init()
        .unwrap();
}
