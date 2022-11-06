use signalrs::client::SignalRClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = SignalRClient::builder("http://localhost:5261/echo")
        .build()
        .await?;

    let result = client
        .call_builder("echo")
        .arg("message")?
        .invoke::<String>()
        .await?;

    println!("result = {}", result);

    Ok(())
}
