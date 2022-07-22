use std::{sync::Arc, task::Poll};
use thiserror::Error;

use flume::r#async::RecvStream;
use futures::{stream::Map, Stream, StreamExt};
use serde::{de::DeserializeOwned, Deserialize};

use crate::{
    error::SignalRError,
    hub::client_sink::ClientSink,
    protocol::{Arguments, ClientStreams},
    request::{HubInvocation, Payload, StreamItemPayload},
};

pub trait FromInvocation
where
    Self: Sized,
{
    fn try_from_request(request: &mut HubInvocation) -> Result<Self, ExtractionError>;
}
// ============= Error

#[derive(Debug, Error)]
pub enum ExtractionError {
    #[error("Arguments not provided the in invocation")]
    MissingArgs,
    #[error("Stream not provided the in invocation")]
    MissingStreamIds,
    #[error("Number of requested client streams exceeds the number of streams in the invocation")]
    NotEnoughStreamIds,
    #[error("JSON deserialization error")]
    JsonError {
        #[from]
        source: serde_json::Error,
    },
    #[error("An error occured : {0}")]
    Other(String),
}

// ============= Args

#[derive(Deserialize, Debug)]
pub struct Args<T>(pub T);

impl<T> FromInvocation for Args<T>
where
    T: DeserializeOwned,
{
    fn try_from_request(request: &mut HubInvocation) -> Result<Self, ExtractionError> {
        match &request.payload {
            Payload::Text(text) => {
                let arguments: Arguments<serde_json::Value> = serde_json::from_str(text.as_str())?;

                let arguments = match arguments.arguments {
                    Some(serde_json::Value::Array(args)) => {
                        if args.len() == 1 {
                            let single = args[0].clone();
                            serde_json::from_value(single)
                        } else if args.len() > 1 {
                            serde_json::from_value(serde_json::Value::Array(args))
                        } else {
                            return Err(ExtractionError::MissingArgs);
                        }
                    }
                    _ => return Err(ExtractionError::MissingArgs),
                }?;

                if let Some(arguments) = arguments {
                    Ok(Args(arguments))
                } else {
                    Err(ExtractionError::MissingArgs)
                }
            }
            _ => unimplemented!(),
        }
    }
}

// ============= ClientStream

pub struct StreamArgs<T: 'static>(pub ClientStream<T>);

pub struct ClientStream<T: 'static> {
    stream: Map<
        RecvStream<'static, StreamItemPayload>,
        fn(StreamItemPayload) -> Result<T, SignalRError>,
    >,
}

impl<T> Stream for ClientStream<T>
where
    T: 'static,
{
    type Item = T;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let next = self.stream.poll_next_unpin(cx);

        match next {
            Poll::Ready(Some(Ok(i))) => Poll::Ready(Some(i)),
            Poll::Ready(Some(Err(_e))) => {
                //TODO: log error!
                todo!()
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<T> FromInvocation for StreamArgs<T>
where
    T: DeserializeOwned,
{
    fn try_from_request(request: &mut HubInvocation) -> Result<Self, ExtractionError> {
        match &request.payload {
            Payload::Text(payload) => {
                let client_streams: ClientStreams = serde_json::from_str(payload)?;

                match client_streams.stream_ids {
                    Some(stream_ids) => {
                        let index = request.pipeline_state.next_stream_id_index;

                        let stream_id = match stream_ids.get(index) {
                            Some(stream_id) => stream_id.clone(),
                            None => return Err(ExtractionError::NotEnoughStreamIds),
                        };

                        let (tx, rx) = flume::bounded::<StreamItemPayload>(100);
                        let (tx, rx) = (tx.into_sink(), rx.into_stream());

                        let client_sink = ClientSink { sink: tx };
                        let client_stream: ClientStream<T> = ClientStream {
                            stream: rx.map(|i| i.try_deserialize::<T>()),
                        };

                        // TODO: Race condtion here between here and hub StreamItem arriving, possible loss of messages
                        tokio::spawn(
                            request
                                .hub_state
                                .client_streams_mapping
                                .clone()
                                .insert(stream_id, client_sink),
                        );

                        request.pipeline_state.next_stream_id_index += 1;

                        return Ok(StreamArgs(client_stream));
                    }
                    None => return Err(ExtractionError::MissingStreamIds),
                }
            }
            Payload::Binary(_) => unimplemented!(),
        }
    }
}
