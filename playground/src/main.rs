mod clients;
mod example_hub;
mod ws_transport;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ws_transport::run(example_hub::HubInvoker::new()).await
}
