use crate::protocol::{Invocation, StreamItem};
use futures::{
    sink::{Sink, SinkExt},
    stream::{Stream, StreamExt},
};
use serde::Serialize;
use std::fmt::Debug;
use thiserror::Error;
use uuid::Uuid;

pub struct SignalRClient<S> {
    sink: S,
}

pub enum InvocationPart<T> {
    Argument(T),
    Stream(InvocationStream<T>),
}

pub struct InvocationStream<T>(Box<dyn Stream<Item = T> + Unpin>);

impl<T> InvocationStream<T> {
    pub fn new(inner: impl Stream<Item = T> + Unpin + 'static) -> Self {
        InvocationStream(Box::new(inner))
    }
}

impl<T> Stream for InvocationStream<T> {
    type Item = T;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.0.poll_next_unpin(cx)
    }
}

#[derive(Error, Debug)]
pub enum SignnalRClientError {
    #[error("Json error")]
    JsonError {
        #[from]
        source: serde_json::Error,
    },
}

pub trait IntoInvocationPart<T> {
    fn into(self) -> InvocationPart<T>;
}

impl<T> IntoInvocationPart<T> for T
where
    T: Serialize,
{
    fn into(self) -> InvocationPart<T> {
        InvocationPart::Argument(self)
    }
}

impl<T> IntoInvocationPart<T> for InvocationStream<T>
where
    T: Serialize,
{
    fn into(self) -> InvocationPart<T> {
        InvocationPart::Stream(self)
    }
}

impl<S> SignalRClient<S>
where
    S: Sink<String> + Unpin,
    <S as Sink<String>>::Error: Debug,
{
    pub async fn send(&mut self, target: String) -> Result<(), SignnalRClientError> {
        self.actually_send(target, None, None).await
    }

    pub async fn send1<T>(&mut self, target: String, arg1: T) -> Result<(), SignnalRClientError>
    where
        T: IntoInvocationPart<T> + Serialize + 'static,
    {
        let t1: InvocationPart<T> = arg1.into();

        match t1 {
            InvocationPart::Argument(a1) => {
                let a1 = serde_json::to_value(a1)?;
                self.actually_send(target, Some(vec![a1]), None).await
            }
            InvocationPart::Stream(s1) => {
                let s1 = s1.map(|x| serde_json::to_value(x).map_err(|x| x.into()));
                self.actually_send(target, None, Some(vec![Box::new(s1)]))
                    .await
            }
        }
    }

    pub async fn send2<T1, T2>(
        &mut self,
        target: String,
        arg1: T1,
        arg2: T2,
    ) -> Result<(), SignnalRClientError>
    where
        T1: IntoInvocationPart<T1> + Serialize + 'static,
        T2: IntoInvocationPart<T2> + Serialize + 'static,
    {
        let mut arguments = Vec::new();
        let mut streams = Vec::new();

        match arg1.into() {
            InvocationPart::Argument(arg) => arguments.push(serde_json::to_value(arg)?),
            InvocationPart::Stream(stream) => {
                streams.push(
                    Box::new(stream.map(|x| serde_json::to_value(x).map_err(|x| x.into())))
                        as Box<dyn Stream<Item = Result<serde_json::Value, SignnalRClientError>>>,
                );
            }
        };

        match arg2.into() {
            InvocationPart::Argument(arg) => arguments.push(serde_json::to_value(arg)?),
            InvocationPart::Stream(stream) => {
                streams.push(Box::new(
                    stream.map(|x| serde_json::to_value(x).map_err(|x| x.into())),
                ));
            }
        };

        let arguments = if arguments.is_empty() {
            None
        } else {
            Some(arguments)
        };

        let streams = if streams.is_empty() {
            None
        } else {
            Some(streams)
        };

        self.actually_send(target, arguments, streams).await
    }

    async fn actually_send(
        &mut self,
        target: String,
        arguments: Option<Vec<serde_json::Value>>,
        streams: Option<
            Vec<Box<dyn Stream<Item = Result<serde_json::Value, SignnalRClientError>>>>,
        >,
    ) -> Result<(), SignnalRClientError> {
        let mut invocation = Invocation::new_non_blocking(target, arguments);

        let mut stream_ids = Vec::new();

        if let Some(streams) = &streams {
            for _ in 0..streams.len() {
                stream_ids.push(Uuid::new_v4().to_string());
            }
            invocation.with_streams(stream_ids.clone());
        }

        self.sink
            .send(serde_json::to_string(&invocation)?)
            .await
            .unwrap(); // TODO: Do not unwrap

        if let Some(streams) = streams {
            for (id, stream) in stream_ids.into_iter().zip(streams) {
                let stream = stream.map(|x| x);
            }
        }

        // stream all streams
        // break the method in case error occurs in any stream
        // send some error message in stream item then?

        todo!()
    }
}
