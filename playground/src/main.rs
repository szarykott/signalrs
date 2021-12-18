mod clients;
mod example_hub;
mod ws_transport;

#[tokio::main]
async fn main() {
    let server = ws_transport::run().await.unwrap();
}
