use async_stream::stream;
use async_trait;
use flume::r#async::{RecvStream, SendSink};
use futures::{Future, Sink, SinkExt, Stream, StreamExt};
use serde;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use signalrs_core::{extensions::StreamExtR, protocol::*};
use std::{any::Any, collections::HashMap, fmt::Debug, sync::Arc};
use tokio;
use tokio::sync::Mutex;

const WEIRD_ENDING: &'static str = "\u{001E}";

pub struct HubInvoker {
    hub: Arc<Hub>,
    ongoing_invocations: Arc<Mutex<HashMap<String, tokio::task::JoinHandle<()>>>>,
    client_streams_mapping: Arc<Mutex<HashMap<String, ClientStream>>>,
}

pub struct ClientStream {
    to_function: String,
    sink: Box<dyn Any + Send>,
}

#[derive(Deserialize, Debug, Clone)]
struct Id {
    #[serde(rename = "invocationId")]
    invocation_id: String,
}

#[derive(Deserialize, Debug, Clone, Copy)]
struct Type {
    #[serde(rename = "type")]
    message_type: MessageType,
}

#[derive(Deserialize, Debug, Clone)]
struct Target {
    target: String,
}

impl HubInvoker {
    pub fn new() -> Self {
        HubInvoker {
            hub: Arc::new(Hub {
                _counter: Arc::new(Mutex::new(0)),
            }),
            ongoing_invocations: Arc::new(Mutex::new(HashMap::new())),
            client_streams_mapping: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn handshake(&self, input: &str) -> String {
        let input = input.trim_end_matches("\u{001E}");

        let request = serde_json::from_str::<HandshakeRequest>(&input);

        let response = match request {
            Ok(request) => {
                if request.is_json() {
                    HandshakeResponse::no_error()
                } else {
                    HandshakeResponse::error("Unsupported protocol")
                }
            }
            Err(e) => HandshakeResponse::error(e),
        };

        match serde_json::to_string(&response) {
            Ok(value) => format!("{}\u{001E}", value),
            Err(e) => "serialization error".to_string(),
        }
    }

    pub async fn invoke_binary(&self, _data: &[u8]) -> Vec<u8> {
        unimplemented!()
    }

    pub async fn invoke_text<T>(
        &self,
        text: String,
        mut output: T,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        T: Sink<String> + Send + 'static + Unpin + Clone,
        <T as Sink<String>>::Error: Debug + std::error::Error,
    {
        let text = text.trim_end_matches("\u{001E}").to_owned();

        match serde_json::from_str::<Type>(&text)?.message_type {
            MessageType::Invocation => {
                let target: Target = serde_json::from_str(&text)?;
                match target.target.as_str() {
                    "non_blocking" => {
                        Self::text_invocation(&text, |_: EmptyArgs| self.hub.non_blocking(), output)
                            .await
                    }
                    "add" => {
                        Self::text_invocation(
                            &text,
                            |arguments: AddArgs| self.hub.add(arguments.0, arguments.1),
                            output,
                        )
                        .await
                    }
                    "single_result_failure" => {
                        Self::text_invocation(
                            &text,
                            |arguments: SingleResultFailureArgs| {
                                self.hub.single_result_failure(arguments.0, arguments.1)
                            },
                            output,
                        )
                        .await
                    }
                    "batched" => {
                        Self::text_invocation(
                            &text,
                            |arguments: BatchedArgs| self.hub.batched(arguments.0),
                            output,
                        )
                        .await
                    }
                    "add_stream" => {
                        let hub = Arc::clone(&self.hub);
                        Self::text_client_stream_invocation(
                            "add_stream",
                            text,
                            move |_: EmptyArgs, b| {
                                let result = async move { hub.add_stream(b).await };
                                HubFutureWrapper(result)
                            },
                            output,
                            Arc::clone(&self.client_streams_mapping),
                        )
                        .await
                    }
                    _ => panic!(),
                }
            }
            MessageType::StreamInvocation => {
                let target: Target = serde_json::from_str(&text)?;
                match target.target.as_str() {
                    "stream" => {
                        Self::text_stream_invocation(
                            &text,
                            |args: StreamArgs| self.hub.stream(args.0),
                            output,
                            Arc::clone(&self.ongoing_invocations),
                        )
                        .await
                    }
                    "stream_failure" => {
                        Self::text_stream_invocation(
                            &text,
                            |args: StreamArgs| self.hub.stream_failure(args.0),
                            output,
                            Arc::clone(&self.ongoing_invocations),
                        )
                        .await
                    }
                    _ => panic!(),
                }
            }
            MessageType::StreamItem => {
                let message: Id = serde_json::from_str(&text)?;

                let mut guard = self.client_streams_mapping.lock().await;
                let cs = (*guard).get_mut(&message.invocation_id);

                if let Some(cs) = cs {
                    match cs.to_function.as_str() {
                        "add_stream" => Self::text_stream_item::<i32>(&text, cs).await?,
                        _ => panic!(),
                    }
                }

                Ok(())
            }
            MessageType::Completion => {
                let message: Id = serde_json::from_str(&text)?;

                let mut guard = self.client_streams_mapping.lock().await;
                let cs = (*guard).remove(&message.invocation_id);

                if let Some(cs) = cs {
                    drop(cs); // should terminate sender
                }

                Ok(())
            }
            MessageType::CancelInvocation => {
                let message: CancelInvocation = serde_json::from_str(&text)?;

                let mut guard = self.ongoing_invocations.lock().await;
                match (*guard).remove(&message.invocation_id) {
                    Some(handle) => handle.abort(),
                    None => { /* all good */ }
                };

                Ok(())
            }
            MessageType::Ping => {
                let ping = Ping::new();
                let s = serde_json::to_string(&ping)?;
                output.send(s + WEIRD_ENDING).await?;
                Ok(())
            }
            MessageType::Close => todo!(),
            MessageType::Other => {
                /* panik or kalm? */
                todo!()
            }
        }
    }

    async fn text_invocation<'de, T, R, F, S>(
        text: &'de str,
        hub_function: F,
        output: S,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        T: Deserialize<'de>,
        F: FnOnce(T) -> R,
        R: HubResponse,
        S: Sink<String> + Send + 'static + Unpin + Clone,
        <S as Sink<String>>::Error: Debug + std::error::Error,
    {
        let mut invocation: Invocation<T> = serde_json::from_str(text)?;

        let arguments = invocation.arguments().unwrap();

        let result = hub_function(arguments);

        match (invocation.id(), result) {
            (Some(id), value) => {
                value.forward(id.clone(), output).await?;
            }
            _ => {}
        }

        Ok(())
    }

    async fn text_stream_invocation<'de, T, R, F, S>(
        text: &'de str,
        hub_function: F,
        output: S,
        invocations: Arc<Mutex<HashMap<String, tokio::task::JoinHandle<()>>>>,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        T: Deserialize<'de>,
        F: FnOnce(T) -> R,
        R: HubResponse + 'static,
        S: Sink<String> + Send + 'static + Unpin + Clone,
        <S as Sink<String>>::Error: Debug + std::error::Error,
    {
        let stream_invocation: StreamInvocation<T> = serde_json::from_str(&text)?;

        let arguments = stream_invocation.arguments.unwrap();
        let invocation_id = stream_invocation.invocation_id;

        let result = hub_function(arguments).forward(invocation_id.clone(), output);

        let invocation_id_clone = invocation_id.clone();
        let invocations_clone = Arc::clone(&invocations);
        let ongoing = tokio::spawn(async move {
            result.await.unwrap();
            let mut invocations = invocations_clone.lock().await;
            (*invocations).remove(&invocation_id_clone);
        });

        let mut guard = invocations.lock().await;
        (*guard).insert(invocation_id, ongoing);

        Ok(())
    }

    async fn text_client_stream_invocation<'de, T, R, F, S, I>(
        function_name: &'static str,
        text: String,
        hub_function: F,
        output: S,
        client_streams_mapping: Arc<Mutex<HashMap<String, ClientStream>>>,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        T: DeserializeOwned + Send + 'static,
        F: FnOnce(T, RecvStream<'static, I>) -> R + Send + 'static,
        I: 'static + Send,
        R: HubResponse + Send + 'static,
        S: Sink<String> + Send + 'static + Unpin + Clone,
        <S as Sink<String>>::Error: Debug + std::error::Error,
    {
        let mut invocation: Invocation<T> = serde_json::from_str(text.as_str())?;

        let invocation_id = invocation.id().clone().unwrap();
        let arguments = invocation.arguments().unwrap();

        if let Some(e) = invocation.stream_ids {
            let (tx, rx) = flume::bounded(100);

            for incoming_stream in e {
                let mut guard = client_streams_mapping.lock().await;
                let cs = ClientStream {
                    to_function: function_name.to_string(),
                    sink: Box::new(tx.clone().into_sink()),
                };
                (*guard).insert(incoming_stream.clone(), cs);
            }

            let output_clone = output.clone();
            let invocation_id_clone = invocation_id.clone();

            tokio::spawn(async move {
                let hub_function_future = hub_function(arguments, rx.into_stream());

                hub_function_future
                    .forward(invocation_id_clone, output_clone)
                    .await
                    .unwrap();
            });
        }

        Ok(())
    }

    async fn text_stream_item<'de, T>(
        text: &'de str,
        cs: &mut ClientStream,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        T: Deserialize<'de> + 'static,
    {
        let item: StreamItem<T> = serde_json::from_str(&text)?;
        let sink = cs.sink.downcast_mut::<SendSink<T>>().unwrap();
        sink.send(item.item).await?;

        Ok(())
    }
}

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

pub struct HubStream;
struct InfallibleHubStream<S>(S);
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

struct HubFutureWrapper<T>(T);

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

pub struct Hub {
    _counter: Arc<Mutex<usize>>,
}

impl Hub {
    pub fn non_blocking(&self) {
        // nothing
    }

    pub fn add(&self, a: i32, b: i32) -> impl HubResponse {
        a + b
    }

    pub fn single_result_failure(&self, _a: u32, _b: u32) -> impl HubResponse {
        Err::<u32, String>("An error!".to_string())
    }

    pub fn batched(&self, count: usize) -> impl HubResponse {
        std::iter::successors(Some(0usize), |p| Some(p + 1))
            .take(count)
            .collect::<Vec<usize>>()
    }

    pub fn stream(&self, count: usize) -> impl HubResponse {
        HubStream::infallible(stream! {
            for i in 0..count {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                yield i;
            }
        })
    }

    pub fn stream_failure(&self, count: usize) -> impl HubResponse {
        HubStream::fallible(stream! {
            for i in 0..count {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                yield Ok(i);
            }
            yield Err("Ran out of data!".to_string())
        })
    }

    pub async fn add_stream(&self, input: impl Stream<Item = i32>) -> impl HubResponse {
        input.collect::<Vec<i32>>().await.into_iter().sum::<i32>()
    }
}

#[derive(Deserialize, Clone, Debug)]
struct BatchedArgs(usize, #[serde(default)] ());

#[derive(Deserialize, Clone, Debug)]
struct AddArgs(i32, i32);

#[derive(Deserialize, Clone, Debug)]
struct SingleResultFailureArgs(u32, u32);

#[derive(Deserialize, Clone, Debug)]
struct StreamArgs(usize, #[serde(default)] ());

#[derive(Deserialize, Clone, Debug)]
struct StreamFailureArgs(usize, #[serde(default)] ());

#[derive(Deserialize, Clone, Debug)]
struct EmptyArgs(#[serde(default)] (), #[serde(default)] ());
