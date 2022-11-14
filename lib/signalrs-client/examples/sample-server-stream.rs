use futures::StreamExt;
use signalrs_client::SignalRClient;
use tracing::*;
use tracing_subscriber::{self, filter, prelude::*};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    set_tracing_subscriber();

    let client = SignalRClient::builder("localhost")
        .use_port(5261)
        .use_hub("streaming")
        .use_unencrypted_connection()
        .build()
        .await?;

    let mut result = client
        .method("AsyncEnumerableCounter")
        .arg(5)?
        .arg(1000)?
        .invoke_stream::<i32>()
        .await?;

    while let Some(Ok(next)) = result.next().await {
        info!("next = {next}");
    }

    info!("result stream finished");

    Ok(())
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
