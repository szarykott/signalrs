//! SignalR invocation builder

use super::{arguments::InvocationArgs, error::ClientError, ResponseStream, SignalRClient};
use crate::{
    arguments::InvocationStream,
    messages::{self, ClientMessage, MessageEncoding},
    protocol::{Invocation, StreamInvocation, StreamItem},
    stream_ext::SignalRStreamExt,
};
use futures::{Stream, StreamExt};
use serde::{de::DeserializeOwned, Serialize};
use uuid::Uuid;

/// Request builder for the [`SignalRClient`]
///
/// Allows adding streams and arguments to the invocation.
/// Contains set of method that drive behavior of response reception.
pub struct InvocationBuilder<'a> {
    client: &'a SignalRClient,
    method: String,
    encoding: MessageEncoding,
    arguments: Vec<serde_json::Value>,
    streams: Vec<ClientStream>,
}

struct ClientStream {
    stream_id: String,
    items: Box<dyn Stream<Item = ClientMessage> + Unpin + Send>,
}

impl<'a> InvocationBuilder<'a> {
    pub(crate) fn new(client: &'a SignalRClient, method: impl ToString) -> Self {
        InvocationBuilder {
            client,
            method: method.to_string(),
            encoding: MessageEncoding::Json,
            arguments: Default::default(),
            streams: Default::default(),
        }
    }

    /// Adds ordered argument to invocation
    ///
    /// Argument can either be:
    /// - a value
    /// - a [stream](futures::stream::Stream)
    ///
    /// Order of arguments matters, they need to be passed in exactly the same order server expects them.
    ///
    /// # Example
    ///
    /// Assuming server has a hub method defined as:
    /// ```rust,no_run
    /// use futures::stream::Stream;
    ///
    /// fn calculate(name: String, a: usize, b: usize, items: impl Stream<Item = usize>) -> usize {
    ///     println!("{}", name);
    ///     a + b
    /// }
    /// ```
    ///
    /// Innvocation would have to be built in a following way to call this method
    /// ```rust,no_run
    /// use signalrs_client::{SignalRClient, arguments::InvocationStream};
    /// use futures::StreamExt;
    ///
    /// # async fn function() -> anyhow::Result<()> {
    /// let client: SignalRClient = get_client();
    /// let result = client.method("calculate")
    ///     .arg("Johnny")?
    ///     .arg(1)?
    ///     .arg(2)?
    ///     .arg(InvocationStream::new(futures::stream::repeat(1usize).take(5)))?   
    ///     .invoke::<usize>()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// # fn get_client() -> SignalRClient { panic!() }
    ///
    /// ```
    pub fn arg<A, B>(mut self, arg: A) -> Result<Self, ClientError>
    where
        A: Into<InvocationArgs<B>> + Send + 'static,
        B: Serialize + Send + 'static,
    {
        match arg.into() {
            InvocationArgs::Argument(arg) => self.arguments.push(
                messages::to_json_value(&arg).map_err(ClientError::malformed_request)?,
            ),
            InvocationArgs::Stream(stream) => {
                let stream_id = Uuid::new_v4().to_string();
                let client_stream = into_client_stream::<B>(stream_id, stream, self.encoding);
                self.streams.push(client_stream);
            }
        };

        return Ok(self);

        fn into_client_stream<A: Serialize + Send + 'static>(
            stream_id: String,
            input: InvocationStream<A>,
            encoding: MessageEncoding,
        ) -> ClientStream {
            let items = input
                .zip(futures::stream::repeat(stream_id.clone()))
                .map(|(i, id)| StreamItem::new(id, i))
                .map(move |i| encoding.serialize(i))
                .append_completion(stream_id.clone(), encoding);

            ClientStream {
                stream_id,
                items: Box::new(items),
            }
        }
    }

    /// Sends an invocation to the server and does not expect any response
    ///
    /// This method follows 'fire and forget' semantics.
    /// As soon as the message is sent from the client it returns to the caller.
    /// Server then processes the request asynchronously.
    pub async fn send(self) -> Result<(), ClientError> {
        let arguments = args_as_option(self.arguments);

        let mut invocation = Invocation::non_blocking(self.method, arguments);
        invocation.with_streams(get_stream_ids(&self.streams));

        let serialized = self
            .encoding
            .serialize(&invocation)
            .map_err(ClientError::malformed_request)?;

        self.client.send_message(serialized).await?;
        SignalRClient::send_streams(
            self.client.get_transport_handle(),
            into_actual_streams(self.streams),
        )
        .await
    }

    /// Sends an ivocation to the server and awaits unit response
    ///
    /// It does not expect any meaingful reponse except from empty response from the server.
    /// It follows semantics such as `void` methods or functions returning `()`.
    pub async fn invoke_unit(self) -> Result<(), ClientError> {
        let invocation_id = Uuid::new_v4().to_string();
        let arguments = args_as_option(self.arguments);

        let mut invocation = Invocation::non_blocking(self.method, arguments);
        invocation.with_invocation_id(invocation_id.clone());
        invocation.with_streams(get_stream_ids(&self.streams));

        let serialized = self
            .encoding
            .serialize(&invocation)
            .map_err(ClientError::malformed_request)?;

        let result = self
            .client
            .invoke_option::<()>(invocation_id, serialized, into_actual_streams(self.streams))
            .await;

        result?;

        Ok(())
    }

    /// Sends an ivocation to the server and awaits meaningful, single response
    ///
    /// It expects the response to be well-structured object in an encoding format used for communication.
    /// For instance this method can return `usize` or `MyCustomStruct` as long as this type implements [`Deserialize`](serde::Deserialize) and is not generic over lifetime.
    ///
    /// # Important
    ///
    /// This function will cause errors if called with `()`. Use [`invoke_unit`](InvocationBuilder::invoke_unit) to do this.
    pub async fn invoke<T: DeserializeOwned>(self) -> Result<T, ClientError> {
        let invocation_id = Uuid::new_v4().to_string();
        let arguments = args_as_option(self.arguments);

        let mut invocation = Invocation::non_blocking(self.method, arguments);
        invocation.with_invocation_id(invocation_id.clone());
        invocation.with_streams(get_stream_ids(&self.streams));

        let serialized = self
            .encoding
            .serialize(&invocation)
            .map_err(ClientError::malformed_request)?;

        self.client
            .invoke_option::<T>(invocation_id, serialized, into_actual_streams(self.streams))
            .await?
            .ok_or_else(|| ClientError::result("expected some result, received empty"))
    }

    /// Sends an ivocation to the server and awaits meaningful stream of responses
    ///
    /// It expects the responses to be well-structured objects in an encoding format used for communication.
    /// For instance this method can return a [`Stream`] of `usize` or `MyCustomStruct` as long as this type implements [`Deserialize`](serde::Deserialize) and is not generic over lifetime.
    ///
    /// # Example
    /// Assuming server has a hub method defined as:
    /// ```rust,no_run
    /// use futures::stream::Stream;
    /// use futures::stream::StreamExt;
    /// fn answers() -> impl Stream<Item = String> {
    ///     // not really important what happens here
    ///     # futures::stream::repeat("congratulation for looking in the source code".to_string()).take(5)
    /// }
    /// ```
    ///
    /// Innvocation would have to be built in a following way to call this method
    /// ```rust,no_run
    /// use signalrs_client::{SignalRClient, arguments::InvocationStream};
    /// use futures::StreamExt;
    ///
    /// # async fn function() -> anyhow::Result<()> {
    /// let client: SignalRClient = get_client();
    /// let mut result = client.method("answers")
    ///     .invoke_stream::<String>().await?;
    ///
    /// while let Some(answer) = result.next().await {
    ///     println!("next answer: {}", answer?);
    /// }
    /// # Ok(())
    /// # }
    /// # fn get_client() -> SignalRClient { panic!() }
    ///
    /// ```
    pub async fn invoke_stream<T: DeserializeOwned>(
        self,
    ) -> Result<ResponseStream<'a, T>, ClientError> {
        let invocation_id = Uuid::new_v4().to_string();

        let mut invocation =
            StreamInvocation::new(invocation_id.clone(), self.method, Some(self.arguments));
        invocation.with_streams(get_stream_ids(&self.streams));

        let serialized = self
            .encoding
            .serialize(&invocation)
            .map_err(ClientError::malformed_request)?;

        let response_stream = self
            .client
            .invoke_stream::<T>(invocation_id, serialized, into_actual_streams(self.streams))
            .await?;

        Ok(response_stream)
    }
}

fn args_as_option(arguments: Vec<serde_json::Value>) -> Option<Vec<serde_json::Value>> {
    if arguments.is_empty() {
        None
    } else {
        Some(arguments)
    }
}

fn get_stream_ids(streams: &[ClientStream]) -> Vec<String> {
    streams.iter().map(|s| s.get_stream_id()).collect()
}

fn into_actual_streams(
    streams: Vec<ClientStream>,
) -> Vec<Box<dyn Stream<Item = ClientMessage> + Unpin + Send>> {
    streams.into_iter().map(|s| s.items).collect()
}

impl ClientStream {
    pub fn get_stream_id(&self) -> String {
        self.stream_id.clone()
    }
}
