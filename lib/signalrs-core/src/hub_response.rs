use crate::{extensions::StreamExtR, protocol::*};
use async_trait;
use futures::{
    sink::{Sink, SinkExt},
    stream::{Stream, StreamExt},
    Future,
};
use serde::Serialize;

const WEIRD_ENDING: &str = "\u{001E}";

#[async_trait::async_trait]
pub trait HubResponse {
    async fn forward<T>(
        self,
        invocation_id: String,
        sink: T,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        T: Sink<String> + Send,
        <T as Sink<String>>::Error: std::error::Error + 'static;
}

#[async_trait::async_trait]
impl HubResponse for () {
    async fn forward<T>(
        self,
        _invocation_id: String,
        _sink: T,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        T: Sink<String> + Send,
        <T as Sink<String>>::Error: std::error::Error + 'static,
    {
        Ok(())
    }
}

macro_rules! impl_hub_response {
    ($($type:ty),+) => {
    $(
        #[async_trait::async_trait]
        impl HubResponse for $type {
            async fn forward<T>(
                self,
                invocation_id: String,
                sink: T,
            ) -> Result<(), Box<dyn std::error::Error>>
            where
                T: Sink<String> + Send,
                <T as Sink<String>>::Error: std::error::Error + 'static,
            {
                let completion = Completion::new(invocation_id, Some(self), None);
                Box::pin(sink)
                    .send(serde_json::to_string(&completion)? + WEIRD_ENDING)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
            }
        }
    )*
    };
}

impl_hub_response!(usize, isize);
impl_hub_response!(i8, i16, i32, i64, i128);
impl_hub_response!(u8, u16, u32, u64, u128);
impl_hub_response!(f32, f64);
impl_hub_response!(String, &'static str);

#[async_trait::async_trait]
impl<R> HubResponse for Vec<R>
where
    R: HubResponse + Send + Serialize,
{
    async fn forward<T>(
        self,
        invocation_id: String,
        sink: T,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        T: Sink<String> + Send,
        <T as Sink<String>>::Error: std::error::Error + 'static,
    {
        let completion = Completion::new(invocation_id, Some(self), None);

        let text = serde_json::to_string(&completion)? + WEIRD_ENDING;

        Box::pin(sink)
            .send(text)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }
}

#[async_trait::async_trait]
impl<R> HubResponse for Result<R, String>
where
    R: HubResponse + Send + Serialize,
{
    async fn forward<T>(
        self,
        invocation_id: String,
        sink: T,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        T: Sink<String> + Send,
        <T as Sink<String>>::Error: std::error::Error + 'static,
    {
        let completion = match self {
            Ok(ok) => Completion::new(invocation_id, Some(ok), None),
            Err(err) => Completion::new(invocation_id, None, Some(err)),
        };

        let text = serde_json::to_string(&completion)? + WEIRD_ENDING;

        Box::pin(sink)
            .send(text)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }
}

// TODO: implement for Result and use to allow user to define their own faillible stream
pub trait Try {}

#[derive(Debug)]
pub struct HubStream;

#[derive(Debug)]
struct InfallibleHubStream<S>(S);

#[derive(Debug)]
struct FallibleHubStream<S>(S);

impl HubStream {
    pub fn infallible<S, I>(stream: S) -> impl HubResponse
    where
        S: Stream<Item = I> + Send + 'static,
        I: Send + Serialize,
    {
        InfallibleHubStream(stream)
    }

    pub fn fallible<S, I>(stream: S) -> impl HubResponse
    where
        S: Stream<Item = Result<I, String>> + Send + 'static,
        I: Send + Serialize,
    {
        FallibleHubStream(stream)
    }
}

#[async_trait::async_trait]
impl<S, I> HubResponse for InfallibleHubStream<S>
where
    S: Stream<Item = I> + Send,
    I: Serialize + Send,
{
    async fn forward<T>(
        self,
        invocation_id: String,
        sink: T,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        T: Sink<String> + Send,
        <T as Sink<String>>::Error: std::error::Error + 'static,
    {
        let result = self.0;

        let responses = result
            .zip(futures::stream::repeat(invocation_id.clone()))
            .map(|(e, id)| StreamItem::new(id, e))
            .map(|si| serde_json::to_string(&si).unwrap() + WEIRD_ENDING)
            .chain(futures::stream::once(async {
                let completion: Completion<usize> = Completion::new(invocation_id, None, None);
                serde_json::to_string(&completion).unwrap() + WEIRD_ENDING
            }));

        let mut responses = Box::pin(responses);
        let mut sink = Box::pin(sink);

        while let Some(item) = responses.next().await {
            sink.send(item)
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl<S, I> HubResponse for FallibleHubStream<S>
where
    S: Stream<Item = Result<I, String>> + Send,
    I: Serialize + Send,
{
    async fn forward<T>(
        self,
        invocation_id: String,
        sink: T,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        T: Sink<String> + Send,
        <T as Sink<String>>::Error: std::error::Error + 'static,
    {
        let result = self.0;

        let responses = result
            .take_while_inclusive(|e| e.is_ok())
            .zip(futures::stream::repeat(invocation_id.clone()))
            .map(|(e, id)| -> Result<StreamItem<I>, Completion<()>> {
                match e {
                    Ok(item) => Ok(StreamItem::new(id, item)),
                    Err(e) => Err(Completion::<()>::new(id, None, Some(e))),
                }
            })
            .chain_if(
                |e| e.is_ok(),
                futures::stream::once(async {
                    let r: Result<StreamItem<I>, Completion<()>> =
                        Err(Completion::<()>::new(invocation_id, None, None));
                    r
                }),
            )
            .map(|e| match e {
                Ok(si) => serde_json::to_string(&si).unwrap() + WEIRD_ENDING,
                Err(cmp) => serde_json::to_string(&cmp).unwrap() + WEIRD_ENDING,
            });

        let mut responses = Box::pin(responses);
        let mut sink = Box::pin(sink);

        while let Some(item) = responses.next().await {
            sink.send(item)
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct HubFutureWrapper<T>(pub T);

#[async_trait::async_trait]
impl<F, O> HubResponse for HubFutureWrapper<F>
where
    F: Future<Output = O> + Send,
    O: HubResponse + Send,
{
    async fn forward<T>(
        self,
        invocation_id: String,
        sink: T,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        T: Sink<String> + Send,
        <T as Sink<String>>::Error: std::error::Error + 'static,
    {
        let result = self.0.await;
        result.forward(invocation_id, sink).await
    }
}