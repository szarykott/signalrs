use flume::r#async::RecvStream;
use futures::{stream::Map, Stream, StreamExt};
use log::*;
use serde::{de::DeserializeOwned, Deserialize};
use std::{fmt::Debug, task::Poll};
use thiserror::Error;

use crate::{
    protocol::{Arguments, ClientStreams},
    server::{
        connection::{ClientSink, StreamItemPayload},
        invocation::{ArgumentsLeft, HubInvocation, Payload},
    },
};

use super::error::SignalRError;

pub trait FromInvocation
where
    Self: Sized,
{
    fn try_from_invocation(request: &mut HubInvocation) -> Result<Self, ExtractionError>;
}
// ============= Error

#[derive(Debug, Error)]
pub enum ExtractionError {
    #[error("Arguments not provided in the invocation")]
    MissingArgs,
    #[error("Stream not provided in the invocation")]
    MissingStreamIds,
    #[error("Number of requested client streams exceeds the number of streams in the invocation")]
    NotEnoughStreamIds,
    #[error("JSON deserialization error")]
    JsonError {
        #[from]
        source: serde_json::Error,
    },
    #[error("Provided arguemnts were not JSON array")]
    NotAnArray,
    #[error("An error occured : {0}")]
    UserDefined(String),
}

// ============= Types

// TODO: macro hygiene!
macro_rules! impl_from_invocation {
    ($($ty:ident),+) => {
        $(
            impl FromInvocation for $ty {
                fn try_from_invocation(request: &mut HubInvocation) -> Result<Self, ExtractionError> {
                    match request.invocation_state.arguments_left {
                        Some(ArgumentsLeft::Text(ref mut values)) => {
                            let next = values.next().ok_or_else(|| ExtractionError::MissingArgs)?;
                            let value = serde_json::from_value(next)?;
                            Ok(value)
                        }
                        Some(ArgumentsLeft::Binary(_)) => unimplemented!(),
                        None => Err(ExtractionError::MissingArgs),
                    }
                }
            }
        )+

    };
}

impl_from_invocation!(u8, u16, u32, u64, u128);
impl_from_invocation!(i8, i16, i32, i64, i128);
impl_from_invocation!(f32, f64);
impl_from_invocation!(usize, isize);
impl_from_invocation!(bool);
impl_from_invocation!(String);

// ============= Args

#[derive(Deserialize, Debug)]
pub struct Args<T>(pub T);

impl<T> FromInvocation for Args<T>
where
    T: DeserializeOwned,
{
    fn try_from_invocation(request: &mut HubInvocation) -> Result<Self, ExtractionError> {
        match &request.payload {
            Payload::Text(text) => {
                let arguments: Arguments<serde_json::Value> = serde_json::from_str(text.as_str())?;

                let arguments = match arguments.arguments {
                    Some(serde_json::Value::Array(mut args)) => {
                        if args.len() == 1 {
                            serde_json::from_value(args[0].take())
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

pub struct UploadStream<T: 'static>(ClientStream<T>);

pub(crate) struct ClientStream<T: 'static> {
    stream: Map<
        RecvStream<'static, StreamItemPayload>,
        fn(StreamItemPayload) -> Result<T, SignalRError>,
    >,
}

impl<T> Stream for UploadStream<T>
where
    T: 'static,
{
    type Item = T;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        self.0.poll_next_unpin(cx)
    }
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
            Poll::Ready(Some(Err(e))) => {
                error!("error in upload stream : {e}");
                Poll::Pending // FIXME: Probably I am wrong
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<T> FromInvocation for UploadStream<T>
where
    T: DeserializeOwned,
{
    fn try_from_invocation(request: &mut HubInvocation) -> Result<Self, ExtractionError> {
        match &request.payload {
            Payload::Text(payload) => {
                let client_streams: ClientStreams = serde_json::from_str(payload)?;

                match client_streams.stream_ids {
                    Some(stream_ids) => {
                        let index = request.invocation_state.next_stream_id_index;

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

                        request
                            .connection_state
                            .upload_sinks
                            .insert(stream_id, client_sink);

                        request.invocation_state.next_stream_id_index += 1;

                        return Ok(UploadStream(client_stream));
                    }
                    None => return Err(ExtractionError::MissingStreamIds),
                }
            }
            Payload::Binary(_) => unimplemented!(),
        }
    }
}
