use serde::{Deserialize, Serialize};

pub const WebSocketTransport: &str = "WebSocket";
pub const TextTransportFormat: &str = "Text";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NegotiateResponseV0 {
    pub connection_id: String,
    pub negotiate_version: u8,
    pub available_transports: Vec<TransportSpec>,
}

impl NegotiateResponseV0 {
    pub fn supported_spec(connection_id: uuid::Uuid) -> Self {
        NegotiateResponseV0 {
            connection_id: connection_id.to_string(),
            negotiate_version: 0,
            available_transports: vec![TransportSpec {
                transport: "WebSockets".into(),
                transfer_formats: vec!["Text".into()],
            }],
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportSpec {
    pub transport: String,
    pub transfer_formats: Vec<String>,
}
