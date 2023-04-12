//! SignalR client builder

use super::{hub::Hub, transport, SignalRClient};
use crate::{
    messages::ClientMessage, protocol::NegotiateResponseV0, transport::error::TransportError,
};
use thiserror::Error;
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tracing::*;

/// [`SignalRClient`] builder.
///
/// Allows configuring connection and behavior details.
///  
/// # Example
/// ```rust, no_run
/// use signalrs_client::SignalRClient;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let client = SignalRClient::builder("example.com")
///         .use_port(8080)
///         .use_hub("echo")
///         .build()
///         .await?;
///
/// # Ok(())
/// }
/// ```
pub struct ClientBuilder {
    domain: String,
    hub: Option<Hub>,
    auth: Auth,
    secure_connection: bool,
    port: Option<usize>,
    query_string: Option<String>,
    hub_path: Option<String>,
}

/// Authentication for negotiate and further network connection
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

/// Errors that can occur during building of the client
#[derive(Error, Debug)]
pub enum BuilderError {
    #[error("negotiate error")]
    Negotiate {
        #[from]
        source: NegotiateError,
    },
    #[error("invalid {0} url")]
    Url(String),
    #[error("transport error")]
    Transport {
        #[from]
        source: TransportError,
    },
}

/// Errors that can occur during negotiating protocol version
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
    #[error("server does not support requested features")]
    Unsupported,
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

    /// Specifies port on the server to connect to.
    pub fn use_port(mut self, port: usize) -> Self {
        self.port = Some(port);
        self
    }

    /// If used, client will use unencrypted connection
    ///
    /// For example HTTP will be used instead of HTTPS and WS instead of WSS.
    /// **Use only when necessary and for local development.**
    pub fn use_unencrypted_connection(mut self) -> Self {
        self.secure_connection = false;
        self
    }

    /// Specifies authentication to use
    pub fn use_authentication(mut self, auth: Auth) -> Self {
        self.auth = auth;
        self
    }

    /// Specifies query string to attch to handshake on the server.
    ///
    /// Since life of a SignalR connection begins with HTTP request it is possible to attach some data in query string.
    /// Some servers would use this data to have initial information about new connection.
    /// There are no standard obligatory parameters, what is obligatory or nice-to-have is dependent of particual hub.
    pub fn use_query_string(mut self, query: String) -> Self {
        self.query_string = Some(query);
        self
    }

    /// Specifies path to a hub on the server to use
    ///
    /// It should be a full path without first `/` e.g `echo` or `full/path/to/echo`.
    pub fn use_hub(mut self, hub: impl ToString) -> Self {
        self.hub_path = Some(hub.to_string());
        self
    }

    /// Specifies a [`Hub`] to use on the client side.
    ///
    /// SignalR allows servers to invoke methods on a client. Pass a hub here to allow server invoking its methods.
    pub fn with_client_hub(mut self, hub: Hub) -> Self {
        self.hub = Some(hub);
        self
    }

    /// Builds an actual clients
    ///
    /// Performs protocol negotiation and server handshake.
    pub async fn build(self) -> Result<SignalRClient, BuilderError> {
        let negotiate_response = self.get_server_supported_features().await?;

        if !can_connect(negotiate_response) {
            return Err(BuilderError::Negotiate {
                source: NegotiateError::Unsupported,
            });
        }

        let mut ws_handle = self.connect_websocket().await?;

        let (tx, rx) = flume::bounded::<ClientMessage>(1);

        let (transport_handle, client) = crate::new_client(tx, self.hub);

        transport::websocket::handshake(&mut ws_handle)
            .await
            .map_err(|error| BuilderError::Transport { source: error })?;

        let transport_future = transport::websocket::websocket_hub(ws_handle, transport_handle, rx);

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

        let (ws_handle, _) = tokio_tungstenite::connect_async(url)
            .await
            .map_err(|error| BuilderError::Transport {
                source: TransportError::Websocket { source: error },
            })?;

        Ok(ws_handle)
    }

    async fn get_server_supported_features(&self) -> Result<NegotiateResponseV0, NegotiateError> {
        let negotiate_endpoint = format!(
            "{}://{}/negotiate?{}",
            self.get_http_scheme(),
            self.get_domain_with_path(),
            self.get_query_string()
        );

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
        match (&self.hub_path, &self.port) {
            (None, None) => self.domain.clone(),
            (None, Some(port)) => format!("{}:{}", self.domain, port),
            (Some(path), None) => format!("{}/{}", self.domain, path),
            (Some(path), Some(port)) => format!("{}:{}/{}", self.domain, port, path),
        }
    }
}

fn can_connect(negotiate_response: NegotiateResponseV0) -> bool {
    negotiate_response
        .available_transports
        .iter()
        .find(|i| i.transport == crate::protocol::WEB_SOCKET_TRANSPORT)
        .and_then(|i| {
            i.transfer_formats
                .iter()
                .find(|j| j.as_str() == crate::protocol::TEXT_TRANSPORT_FORMAT)
        })
        .is_some()
}
