use signalrs_client::SignalRClient;
use tracing::*;
use tracing_subscriber::{self, filter, prelude::*};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    set_tracing_subscriber();

    let client = SignalRClient::builder("localhost")
        .use_port(5261)
        .use_hub("echo")
        .use_unencrypted_connection()
        .build()
        .await?;

    let result = client
        .method("echo")
        .arg("message")?
        .invoke::<String>()
        .await?;

    info!("result = {}", result);

    assert_eq!("message", result);

    client
        .method("NoEcho")
        .arg("message")?
        .invoke_unit()
        .await?;

    client.method("NoEcho").arg("message")?.send().await?;

    Ok(())
}

fn set_tracing_subscriber() {
    let targets_filter = filter::Targets::new()
        .with_target("signalrs", Level::TRACE)
        .with_target("tokio_tungstenite::compat", Level::DEBUG)
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
