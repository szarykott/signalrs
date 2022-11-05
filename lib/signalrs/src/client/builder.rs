use super::{
    client2::{self, SignalRClient},
    hub::Hub,
    messages::MessageEncoding,
    websocket, ClientMessage,
};
use crate::negotiate::NegotiateResponseV0;
use futures::SinkExt;
use thiserror::Error;
use tokio_tungstenite::tungstenite;

pub struct ClientBuilder {
    url: String,
    hub: Option<Hub>,
    encoding: MessageEncoding,
    auth: Auth,
}

pub enum Auth {
    None,
    Basic {
        user: String,
        password: Option<String>,
    },
    Bearer {
        token: String,
    },
}

#[derive(Error, Debug)]
pub enum BuilderError {
    #[error("negotiate error")]
    Negotiate {
        #[from]
        source: NegotiateError,
    },
    #[error("invalid {0} url")]
    Url(String),
    #[error("websocket error")]
    Websocket {
        #[from]
        source: tungstenite::Error,
    },
}

#[derive(Error, Debug)]
pub enum NegotiateError {
    #[error("request error")]
    Request {
        #[from]
        source: reqwest::Error,
    },
    #[error("deserialization error")]
    Deserialization {
        #[from]
        source: serde_json::Error,
    },
}

impl ClientBuilder {
    pub fn new(url: impl Into<String>) -> Self {
        ClientBuilder {
            url: url.into(),
            encoding: MessageEncoding::Json,
            hub: None,
            auth: Auth::None,
        }
    }

    pub fn with_hub(&mut self, hub: Hub) -> &mut Self {
        self.hub = Some(hub);
        self
    }

    pub fn with_authentication(&mut self, auth: Auth) -> &mut Self {
        self.auth = auth;
        self
    }

    pub async fn build(self) -> Result<SignalRClient, BuilderError> {
        let negotiate_response = self.get_server_supported_features().await?;

        if !can_connect(negotiate_response) {
            todo!() // return error
        }

        let (ws_handle, _) = tokio_tungstenite::connect_async(to_ws_scheme(&self.url)?).await?;
        let (tx, rx) = flume::bounded::<ClientMessage>(1);

        let (transport_handle, client) = client2::new_client(tx, self.hub);
        let transport_future = websocket::websocket_hub(ws_handle, transport_handle, rx);

        tokio::spawn(transport_future);

        Ok(client)
    }

    async fn get_server_supported_features(&self) -> Result<NegotiateResponseV0, NegotiateError> {
        let negotiate_endpoint = if self.url.ends_with("/") {
            format!("{}{}", &self.url, "negotiate")
        } else {
            format!("{}/{}", &self.url, "negotiate")
        };

        let mut request = reqwest::Client::new().get(negotiate_endpoint);

        request = match &self.auth {
            Auth::None => request,
            Auth::Basic { user, password } => request.basic_auth(user, password.clone()),
            Auth::Bearer { token } => request.bearer_auth(token),
        };

        let http_response = request.send().await?.error_for_status()?;

        let response: NegotiateResponseV0 = serde_json::from_str(&http_response.text().await?)?;

        Ok(response)
    }
}

fn to_ws_scheme(url: &str) -> Result<String, BuilderError> {
    if url.starts_with("https://") {
        Ok(url.replace("https://", "wss://"))
    } else if url.starts_with("http://") {
        Ok(url.replace("http://", "ws://"))
    } else {
        Err(BuilderError::Url(url.to_owned()))
    }
}

fn can_connect(negotiate_response: NegotiateResponseV0) -> bool {
    negotiate_response
        .available_transports
        .iter()
        .find(|i| i.transport == crate::negotiate::WebSocketTransport)
        .and_then(|i| {
            i.transfer_formats
                .iter()
                .find(|j| j.as_str() == crate::negotiate::TextTransportFormat)
        })
        .is_some()
}
