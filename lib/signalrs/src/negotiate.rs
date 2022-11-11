use serde::{Deserialize, Serialize};

pub const WEB_SOCKET_TRANSPORT: &str = "WebSockets";
pub const TEXT_TRANSPORT_FORMAT: &str = "Text";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NegotiateResponseV0 {
    pub connection_id: String,
    pub negotiate_version: u8,
    pub available_transports: Vec<TransportSpec>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportSpec {
    pub transport: String,
    pub transfer_formats: Vec<String>,
}

impl NegotiateResponseV0 {
    pub fn supported_spec(connection_id: uuid::Uuid) -> Self {
        NegotiateResponseV0 {
            connection_id: connection_id.to_string(),
            negotiate_version: 0,
            available_transports: vec![TransportSpec {
                transport: WEB_SOCKET_TRANSPORT.into(),
                transfer_formats: vec![TEXT_TRANSPORT_FORMAT.into()],
            }],
        }
    }
}
