use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NegotiateResponseV0 {
    connection_id: String,
    negotiate_version: u8,
    available_transports: Vec<TransportSpec>,
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportSpec {
    transport: String,
    transfer_formats: Vec<String>,
}
