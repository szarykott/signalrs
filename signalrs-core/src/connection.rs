use crate::protocol::{self, Message, MessageFormat, Object};
use futures::{
    sink::{Sink, SinkExt},
    stream::{Stream, StreamExt},
};
use signalrs_error::SignalRError;

pub trait MessageSink: Sink<Message, Error = SignalRError> + Unpin {}
pub trait MessageStream: Stream<Item = Message> + Unpin {}

#[derive(Debug)]
pub(crate) struct SignalRConnection<I, O> {
    input: I,
    output: O,
}

pub(crate) struct SignalRServerState;
pub(crate) struct SignalRClientState;

#[derive(Debug)]
pub(crate) struct SignalRSink<O, S> {
    output: O,
    state: S,
}

#[derive(Debug)]
pub(crate) struct SignalRStream<I> {
    input: I,
}

impl<I, O> SignalRConnection<I, O>
where
    I: MessageStream,
    O: MessageSink,
{
    // pub async fn initialize_client(
    //     mut input: I,
    //     mut output: O,
    //     format: MessageFormat,
    // ) -> Result<SignalRConnection<I, O, EstablishedSignalRClientConnection>, SignalRError> {
    //     output.send(Message::HandshakeRequest(None, format)).await?;

    //     match input.next().await {
    //         Some(Message::HandshakeResponse(_, None)) => Ok(SignalRConnection {
    //             state: EstablishedSignalRClientConnection,
    //             input,
    //             output,
    //         }),
    //         Some(Message::HandshakeResponse(_, Some(e))) => Err(SignalRError::Transport(e)), // TODO: error
    //         _ => Err(SignalRError::Transport("".to_string())),
    //     }
    // }

    // pub async fn initialize_server(
    //     mut input: I,
    //     mut output: O,
    //     supported_formats: HashSet<MessageFormat>,
    // ) -> Result<(SignalRSink<O, SignalRServerState>, SignalRStream<I>), SignalRError> {
    //     match input.next().await {
    //         Some(HandshakeMessage::HandshakeRequest(_, format)) => {
    //             if supported_formats.contains(&format) {
    //                 output.send(Message::HandshakeResponse(None, None)).await?;
    //                 Ok((SignalRSink { output, state: SignalRServerState }, SignalRStream { input }))
    //             } else {
    //                 output
    //                     .send(Message::HandshakeResponse(
    //                         None,
    //                         Some("Unsupported format requested".to_string()),
    //                     ))
    //                     .await?;
    //                 Err(SignalRError::ProtocolViolation(
    //                     "Unsupported format requested".to_string(),
    //                 )) // TODO: error
    //             }
    //         }
    //         _ => Err(SignalRError::Transport(
    //             "No message received from client".to_string(),
    //         )),
    //     }
    // }
}

// Some actions are allowed on both server and client ...
impl<O, S> SignalRSink<O, S>
where
    O: MessageSink,
{
    pub async fn stream_item(
        &mut self,
        invocation_id: String,
        item: Object,
    ) -> Result<(), SignalRError> {
        self.output
            .send(protocol::StreamItem::new(invocation_id, item).into())
            .await
    }

    pub async fn completion(
        &mut self,
        invocation_id: String,
        result: Option<Object>,
        error: Option<String>,
    ) -> Result<(), SignalRError> {
        self.output
            .send(protocol::Completion::new(invocation_id, result, error).into())
            .await
    }

    pub async fn ping(&mut self) -> Result<(), SignalRError> {
        self.output.send(protocol::Ping::new().into()).await
    }

    pub async fn close(
        mut self,
        error: Option<String>,
        allow_reconnect: Option<bool>,
    ) -> Result<(), SignalRError> {
        self.output
            .send(protocol::Close::new(error, allow_reconnect).into())
            .await
    }
}

// And some actions are only allowed on client ...
impl<O> SignalRSink<O, SignalRClientState> where O: MessageSink {}

impl<I> SignalRStream<I>
where
    I: MessageStream,
{
    pub async fn receive(&mut self) -> Option<Message> {
        self.input.next().await
    }
}
