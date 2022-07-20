use std::{marker::PhantomData, sync::Arc, task::Poll};

use flume::r#async::RecvStream;
use futures::{stream::Map, FutureExt, Stream, StreamExt};
use serde::{de::DeserializeOwned, Deserialize};
use tokio::sync::MutexGuard;

use crate::{
    error::SignalRError,
    hub::ClientSink,
    protocol::{Arguments, ClientStreams},
    request::{HubRequest, Payload, StreamItemPayload},
};

pub trait FromRequest
where
    Self: Sized,
{
    fn try_from_request(request: &mut HubRequest) -> Result<Self, SignalRError>;
}

// ============= Args

#[derive(Deserialize, Debug)]
pub struct Args<T>(pub T);

impl<T> FromRequest for Args<T>
where
    T: DeserializeOwned,
{
    fn try_from_request(request: &mut HubRequest) -> Result<Self, SignalRError> {
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
                            return Err(SignalRError::UnnspecifiedError);
                        }
                    }
                    _ => return Err(SignalRError::UnnspecifiedError),
                }?;

                if let Some(arguments) = arguments {
                    Ok(Args(arguments))
                } else {
                    Err(SignalRError::UnnspecifiedError)
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

impl<T> FromRequest for StreamArgs<T>
where
    T: DeserializeOwned,
{
    fn try_from_request(request: &mut HubRequest) -> Result<Self, SignalRError> {
        match &request.payload {
            Payload::Text(payload) => {
                let client_streams: ClientStreams = serde_json::from_str(payload)?;

                match client_streams.stream_ids {
                    Some(stream_ids) => {
                        let index = request.pipeline_state.next_stream_id_index;

                        let stream_id = match stream_ids.get(index) {
                            Some(stream_id) => stream_id.clone(),
                            None => return Err(SignalRError::UnnspecifiedError),
                        };

                        let (tx, rx) = flume::bounded::<StreamItemPayload>(100);
                        let (tx, rx) = (tx.into_sink(), rx.into_stream());

                        let client_stream: ClientStream<T> = ClientStream {
                            stream: rx.map(|i| i.try_deserialize::<T>()),
                        };
                        let client_sink = ClientSink { sink: tx };

                        // TODO: Race condtion here between here and hub StreamItem arriving, possible loss of messages
                        let mappings = Arc::clone(&request.hub_state.client_streams_mapping);
                        let fut = async move {
                            let mut mappings = mappings.lock().await;
                            (*mappings).insert(stream_id, client_sink);
                        };

                        tokio::spawn(fut);

                        request.pipeline_state.next_stream_id_index += 1;

                        return Ok(StreamArgs(client_stream));
                    }
                    None => return Err(SignalRError::UnnspecifiedError),
                }
            }
            Payload::Binary(_) => unimplemented!(),
        }
    }
}
