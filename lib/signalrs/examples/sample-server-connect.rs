use signalrs::client::SignalRClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = SignalRClient::builder("http://localhost:5261/echo")
        .build()
        .await?;

    let result = client.call_builder("echo").arg("message")?.send().await?;

    Ok(())
}
