use super::{
    client::{self, SignalRClient},
    hub::Hub,
    websocket,
};
use crate::{client::messages::ClientMessage, negotiate::NegotiateResponseV0};
use thiserror::Error;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    tungstenite::{self, WebSocket},
    MaybeTlsStream, WebSocketStream,
};
use tracing::*;

pub struct ClientBuilder {
    domain: String,
    hub: Option<Hub>,
    auth: Auth,
    secure_connection: bool,
    port: Option<usize>,
    query_string: Option<String>,
    hub_path: Option<String>,
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
    pub fn new(domain: impl ToString) -> Self {
        ClientBuilder {
            domain: domain.to_string(),
            hub: None,
            auth: Auth::None,
            secure_connection: true,
            port: None,
            query_string: None,
            hub_path: None,
        }
    }

    pub fn use_port(mut self, port: usize) -> Self {
        self.port = Some(port);
        self
    }

    pub fn use_unencrypted_connection(mut self) -> Self {
        self.secure_connection = false;
        self
    }

    pub fn use_authentication(&mut self, auth: Auth) -> &mut Self {
        self.auth = auth;
        self
    }

    pub fn use_query_string(mut self, query: String) -> Self {
        self.query_string = Some(query);
        self
    }

    pub fn use_hub(mut self, hub: String) -> Self {
        self.hub_path = Some(hub);
        self
    }

    pub fn with_client_hub(mut self, hub: Hub) -> Self {
        self.hub = Some(hub);
        self
    }

    pub async fn build(self) -> Result<SignalRClient, BuilderError> {
        let negotiate_response = self.get_server_supported_features().await?;

        if !can_connect(negotiate_response) {
            todo!() // return error
        }

        let mut ws_handle = self.connect_websocket().await?;

        let (tx, rx) = flume::bounded::<ClientMessage>(1);

        let (transport_handle, client) = client::new_client(tx, self.hub);

        websocket::handshake(&mut ws_handle).await.unwrap(); // TODO: no unwrap

        let transport_future = websocket::websocket_hub(ws_handle, transport_handle, rx);

        tokio::spawn(transport_future);

        event!(Level::DEBUG, "constructed client");

        Ok(client)
    }

    async fn connect_websocket(
        &self,
    ) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, BuilderError> {
        let scheme = self.get_ws_scheme();
        let domain_and_path = self.get_domain_with_path();
        let query = self.get_query_string();

        let url = format!("{}://{}?{}", scheme, domain_and_path, query);

        let (ws_handle, _) = tokio_tungstenite::connect_async(url).await?;

        Ok(ws_handle)
    }

    async fn get_server_supported_features(&self) -> Result<NegotiateResponseV0, NegotiateError> {
        let scheme = self.get_http_scheme();
        let negotiate_endpoint = format!("{}://{}/negotiate", scheme, self.get_domain_with_path());

        let mut request = reqwest::Client::new().post(negotiate_endpoint);

        request = match &self.auth {
            Auth::None => request,
            Auth::Basic { user, password } => request.basic_auth(user, password.clone()),
            Auth::Bearer { token } => request.bearer_auth(token),
        };

        let http_response = request.send().await?.error_for_status()?;

        let response: NegotiateResponseV0 = serde_json::from_str(&http_response.text().await?)?;

        Ok(response)
    }

    fn get_query_string(&self) -> String {
        if let Some(qs) = &self.query_string {
            qs.clone()
        } else {
            Default::default()
        }
    }

    fn get_http_scheme(&self) -> &str {
        if self.secure_connection {
            "https"
        } else {
            "http"
        }
    }

    fn get_ws_scheme(&self) -> &str {
        if self.secure_connection {
            "wss"
        } else {
            "ws"
        }
    }

    fn get_domain_with_path(&self) -> String {
        if let Some(path) = &self.hub_path {
            format!("{}/{}", &self.domain, path)
        } else {
            format!("{}", &self.domain)
        }
    }
}

fn can_connect(negotiate_response: NegotiateResponseV0) -> bool {
    negotiate_response
        .available_transports
        .iter()
        .find(|i| i.transport == crate::negotiate::WEB_SOCKET_TRANSPORT)
        .and_then(|i| {
            i.transfer_formats
                .iter()
                .find(|j| j.as_str() == crate::negotiate::TEXT_TRANSPORT_FORMAT)
        })
        .is_some()
}
