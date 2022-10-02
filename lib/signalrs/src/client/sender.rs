use crate::protocol::{Completion, Invocation, StreamItem};
use futures::{
    sink::{Sink, SinkExt},
    stream::{FuturesUnordered, Stream, StreamExt},
};
use serde::Serialize;
use uuid::Uuid;

use super::SignalRClientError;

pub struct SignalRClientSender<S> {
    pub(super) sink: S,
}

macro_rules! send_text {
    ($name:ident, $($ty:ident),+) => {
        #[allow(non_snake_case)]
        pub async fn $name<$($ty,)+>(
            &mut self,
            target: String,
            invocation_id: Option<String>,
            $(
                $ty: $ty,
            )+
        ) -> Result<(), SignalRClientError>
        where
        $(
            $ty: IntoInvocationPart<$ty> + Serialize + 'static,
        )+
        {
            let mut arguments = Vec::new();
            let mut streams = Vec::new();

            $(
                match $ty.into() {
                    InvocationPart::Argument(arg) => arguments.push(serde_json::to_value(arg)?),
                    InvocationPart::Stream(stream) => {
                        streams.push(
                            Box::new(stream.map(|x| serde_json::to_value(x).map_err(|x| x.into())))
                                as Box<
                                    dyn Stream<Item = Result<serde_json::Value, SignalRClientError>>
                                        + Unpin,
                                >,
                        );
                    }
                };
            )+

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

            self.actually_send_text(target, invocation_id, arguments, streams)
                .await
        }
    };
}

impl<S> SignalRClientSender<S>
where
    S: Sink<String, Error = SignalRClientError> + Unpin + Clone,
{
    pub async fn send_text0(
        &mut self,
        target: String,
        invocation_id: Option<String>,
    ) -> Result<(), SignalRClientError> {
        self.actually_send_text(target, invocation_id, None, None)
            .await
    }

    pub async fn send_text1<T>(
        &mut self,
        target: String,
        invocation_id: Option<String>,
        arg1: T,
    ) -> Result<(), SignalRClientError>
    where
        T: IntoInvocationPart<T> + Serialize + 'static,
    {
        let t1: InvocationPart<T> = arg1.into();

        match t1 {
            InvocationPart::Argument(a1) => {
                let a1 = serde_json::to_value(a1)?;
                self.actually_send_text(target, invocation_id, Some(vec![a1]), None)
                    .await
            }
            InvocationPart::Stream(s1) => {
                let s1 = s1.map(|x| serde_json::to_value(x).map_err(|x| x.into()));
                self.actually_send_text(target, invocation_id, None, Some(vec![Box::new(s1)]))
                    .await
            }
        }
    }

    send_text!(send_text2, T1, T2);
    send_text!(send_text3, T1, T2, T3);
    send_text!(send_text4, T1, T2, T3, T4);
    send_text!(send_text5, T1, T2, T3, T4, T5);
    send_text!(send_text6, T1, T2, T3, T4, T5, T6);
    send_text!(send_text7, T1, T2, T3, T4, T5, T6, T7);
    send_text!(send_text8, T1, T2, T3, T4, T5, T6, T7, T8);
    send_text!(send_text9, T1, T2, T3, T4, T5, T6, T7, T8, T9);
    send_text!(send_text10, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
    send_text!(send_text11, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
    send_text!(
        send_text12,
        T1,
        T2,
        T3,
        T4,
        T5,
        T6,
        T7,
        T8,
        T9,
        T10,
        T11,
        T12
    );
    send_text!(
        send_text13,
        T1,
        T2,
        T3,
        T4,
        T5,
        T6,
        T7,
        T8,
        T9,
        T10,
        T11,
        T12,
        T13
    );

    async fn actually_send_text(
        &mut self,
        target: String,
        invocation_id: Option<String>,
        arguments: Option<Vec<serde_json::Value>>,
        streams: Option<
            Vec<Box<dyn Stream<Item = Result<serde_json::Value, SignalRClientError>> + Unpin>>,
        >,
    ) -> Result<(), SignalRClientError> {
        let mut invocation = Invocation::new_non_blocking(target, arguments);

        if let Some(id) = invocation_id {
            invocation.add_invocation_id(id);
        }

        let mut stream_ids = Vec::new();

        if let Some(streams) = &streams {
            for _ in 0..streams.len() {
                stream_ids.push(Uuid::new_v4().to_string());
            }
            invocation.with_streams(stream_ids.clone());
        }

        self.sink.send(serde_json::to_string(&invocation)?).await?;

        if let Some(streams) = streams {
            let mut futures = FuturesUnordered::new();

            for (id, stream) in stream_ids.into_iter().zip(streams) {
                let sink = self.sink.clone();
                let future = async move {
                    match Self::stream_it(sink, id.as_str(), stream).await {
                        Ok(()) => Ok(()),
                        Err(error) => Err((id, error)),
                    }
                };

                futures.push(future);
            }

            while let Some(result) = futures.next().await {
                if let Err((id, error)) = result {
                    let completion = Completion::<()>::error(id, error.to_string());
                    self.sink.send(serde_json::to_string(&completion)?).await?;
                    return Err(error); // TODO: return here? client might still be interested in the rest of streams
                }
            }
        }

        Ok(())
    }

    async fn stream_it(
        mut sink: S,
        invocation_id: &str,
        mut stream: Box<dyn Stream<Item = Result<serde_json::Value, SignalRClientError>> + Unpin>,
    ) -> Result<(), SignalRClientError> {
        loop {
            match stream.next().await {
                Some(Ok(value)) => {
                    let stream_item = StreamItem::new(invocation_id, value);
                    sink.send(serde_json::to_string(&stream_item)?).await?;
                }
                Some(Err(error)) => return Err(error),
                None => return Ok(()),
            }
        }
    }
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

// ($($ty:ident),+)
