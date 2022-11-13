use serde::Deserialize;
use signalrs_client::{hub::Hub, SignalRClient};
use signalrs_derive::HubArgument;
use std::time::Duration;
use tracing::*;
use tracing_subscriber::{self, filter, prelude::*};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    set_tracing_subscriber();

    let client1 = get_client("client1".into()).await?;

    client1
        .method("Send")
        .arg("client1")?
        .arg("a message")?
        .send()
        .await?;

    let client2 = get_client("client2".into()).await?;

    client2
        .method("Send")
        .arg("client2")?
        .arg("a message")?
        .send()
        .await?;

    tokio::time::sleep(Duration::from_secs(5)).await;

    Ok(())
}

async fn get_client(name: String) -> anyhow::Result<SignalRClient> {
    let hub = Hub::default().method("Send", print).method("Test", test);

    let client = SignalRClient::builder("localhost:5261")
        .use_port(5261)
        .use_hub("chat")
        .use_unencrypted_connection()
        .use_query_string(format!("name={name}"))
        .with_client_hub(hub)
        .build()
        .await?;

    Ok(client)
}

fn set_tracing_subscriber() {
    let targets_filter = filter::Targets::new()
        .with_target("signalrs", Level::TRACE)
        .with_default(Level::DEBUG);

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_line_number(false)
        .with_file(false)
        .without_time()
        .compact();

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(targets_filter)
        .init();
}

async fn print(message: String) {
    info!("{message}");
}

async fn test(arg: SomeStruct) {
    println!("{:#?}", arg);
}

#[derive(Debug, HubArgument, Deserialize)]
struct SomeStruct {
    data: i32,
    data2: String,
}
