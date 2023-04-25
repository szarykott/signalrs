use signalrs_client::SignalRClient;

#[tokio::test]
async fn main() -> anyhow::Result<()> {
    let client = SignalRClient::builder("localhost")
        .use_port(5261)
        .use_hub("echo")
        .use_unencrypted_connection()
        .build()
        .await?;

    client
        .method("NoEcho")
        .arg("message")?
        .invoke_unit()
        .await?;

    let result = client
        .method("echo")
        .arg("message")?
        .invoke::<String>()
        .await?;

    assert_eq!("message", result);

    Ok(())
}
