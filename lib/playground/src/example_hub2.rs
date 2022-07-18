use async_stream::stream;
use flume::r#async::SendSink;
use futures::{future, Future, Sink, SinkExt, Stream, StreamExt};
use serde;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use signalrs_core::{error::SignalRError, hub_response::*, protocol::*};
use std::marker::PhantomData;
use std::pin::Pin;
use std::{any::Any, collections::HashMap, fmt::Debug, sync::Arc};
use tokio;
use tokio::sync::Mutex;

const WEIRD_ENDING: &str = "\u{001E}";

pub struct HubInvoker {
    ongoing_invocations: Arc<Mutex<HashMap<String, tokio::task::JoinHandle<()>>>>,
    client_streams_mapping: Arc<Mutex<HashMap<String, ClientStream>>>,
}

pub struct ClientStream {
    to_function: String,
    sink: Box<dyn Any + Send>,
}

pub trait Handler<T> {
    type Future: Future<Output = Result<(), SignalRError>> + Send;
    type Sink;

    fn call(self, request: String, output: Self::Sink, stream: bool) -> Self::Future;
}

pub trait SingleResponse<Ret> {
    fn into_completion() -> Completion<Ret>;
}

mod private {
    pub trait Sealed {}
}

#[async_trait::async_trait]
pub trait StreamingResponse<Ret>: private::Sealed {
    async fn stream_it();
}

impl<F, Ret, T1> Handler<T1> for F
where
    F: FnOnce(T1) -> Ret,
    Ret: HubResponse + Send + 'static,
    T1: DeserializeOwned + 'static,
{
    type Future = Pin<Box<dyn Future<Output = Result<(), SignalRError>> + Send + Sync>>;
    type Sink = ResponseSink;

    fn call(self, request: String, output: Self::Sink, stream: bool) -> Self::Future {
        if stream {
            unimplemented!()
        } else {
            let mut invocation: Invocation<_> = serde_json::from_str(&request).unwrap();
            let result = (self)(invocation.arguments().unwrap()); // TODO: Unwrap necessary?
            if let Some(id) = invocation.id() {
                let id_clone = id.clone();
                return Box::pin(async move { result.forward(id_clone, output).await });
            }
        }
        Box::pin(future::ready(Ok(())))
    }
}

pub struct IntoCallable<H, T> {
    handler: H,
    _marker: PhantomData<T>,
}

impl<H, T> IntoCallable<H, T> {
    pub fn new(handler: H) -> Self {
        IntoCallable {
            handler,
            _marker: Default::default(),
        }
    }
}

pub trait Callable {
    type Future: Future<Output = Result<(), SignalRError>> + Send;
    type Sink;

    fn call(&self, request: String, output: Self::Sink, stream: bool) -> Self::Future;
}

impl<H, T> Callable for IntoCallable<H, T>
where
    H: Handler<T> + Clone,
{
    type Future = <H as Handler<T>>::Future;
    /// need to return concrete future different from handler?
    type Sink = <H as Handler<T>>::Sink;

    fn call(&self, request: String, output: Self::Sink, stream: bool) -> Self::Future {
        let handler = self.handler.clone();
        handler.call(request, output, stream)
    }
}

// ======== Builder

pub struct HubBuilder {
    methods: HashMap<
        String,
        Arc<
            dyn Callable<
                Future = Pin<Box<dyn Future<Output = Result<(), SignalRError>> + Send + Sync>>,
                Sink = ResponseSink,
            >,
        >,
    >,
}

impl HubBuilder {
    pub fn new() -> Self {
        HubBuilder {
            methods: Default::default(),
        }
    }

    pub fn method<H, Args>(mut self, name: &str, handler: H) -> Self
    where
        H: Handler<
                Args,
                Future = Pin<Box<dyn Future<Output = Result<(), SignalRError>> + Send + Sync>>,
                Sink = ResponseSink,
            >
            + 'static
            + Clone,
        Args: DeserializeOwned + 'static,
    {
        let callable: IntoCallable<_, Args> = IntoCallable::new(handler);
        self.methods.insert(name.to_owned(), Arc::new(callable));
        self
    }

    pub fn build(self) -> Hub {
        Hub {
            methods: self.methods,
        }
    }
}

pub struct Hub {
    methods: HashMap<
        String,
        Arc<
            dyn Callable<
                Future = Pin<Box<dyn Future<Output = Result<(), SignalRError>> + Send + Sync>>,
                Sink = ResponseSink,
            >,
        >,
    >,
}

impl Hub {
    pub fn handshake(&self, input: &str) -> String {
        let input = input.trim_end_matches(WEIRD_ENDING);

        let request = serde_json::from_str::<HandshakeRequest>(input);

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
            Ok(value) => format!("{}{}", value, WEIRD_ENDING),
            Err(e) => e.to_string(),
        }
    }

    pub async fn invoke_text<T>(
        &self,
        text: String,
        output: ResponseSink,
    ) -> Result<(), SignalRError> {
        let text = text.trim_end_matches(WEIRD_ENDING).to_owned();

        match serde_json::from_str::<Type>(&text)?.message_type {
            MessageType::Invocation => {
                let target: Target = serde_json::from_str(&text)?;

                if let Some(callable) = self.methods.get(&target.target) {
                    callable.call(text, output, false).await?;
                }
            }
            _ => panic!(),
        };

        Ok(())
    }
}

#[allow(dead_code)]
fn test() {
    let builder = HubBuilder::new()
        .method("identity", identity)
        .method("identity2", identity)
        .build();
}

// ======== Handshake

impl HubInvoker {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        HubInvoker {
            ongoing_invocations: Arc::new(Mutex::new(HashMap::new())),
            client_streams_mapping: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn handshake(&self, input: &str) -> String {
        let input = input.trim_end_matches(WEIRD_ENDING);

        let request = serde_json::from_str::<HandshakeRequest>(input);

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
            Ok(value) => format!("{}{}", value, WEIRD_ENDING),
            Err(e) => e.to_string(),
        }
    }
}

// ========= Operation

impl HubInvoker {
    pub async fn invoke_text<T>(
        &self,
        text: String,
        mut output: T,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        T: Sink<String> + Send + 'static + Unpin + Clone,
        <T as Sink<String>>::Error: Debug + std::error::Error,
    {
        let text = text.trim_end_matches(WEIRD_ENDING).to_owned();

        match serde_json::from_str::<Type>(&text)?.message_type {
            MessageType::Invocation => {
                let target: Target = serde_json::from_str(&text)?;

                match target.target.as_str() {
                    "non_blocking" => {
                        let invocation: Invocation<()> = serde_json::from_str(&text)?;

                        let result = non_blocking();

                        if let Some(id) = invocation.id() {
                            // result.forward(id.clone(), output).await?;
                            todo!()
                        }

                        Ok(())
                    }
                    "add" => {
                        let mut invocation: Invocation<_> = serde_json::from_str(&text)?;

                        let arguments = invocation.arguments().unwrap();

                        let result = add(arguments).into_stream();

                        if let Some(id) = invocation.id() {
                            // result.forward(id.clone(), output).await?;
                            todo!()
                        }

                        Ok(())
                    }
                    "single_result_failure" => {
                        let mut invocation: Invocation<_> = serde_json::from_str(&text)?;

                        let arguments = invocation.arguments().unwrap();

                        let result = single_result_failure(arguments);

                        if let Some(id) = invocation.id() {
                            // result.forward(id.clone(), output).await?;
                            todo!()
                        }

                        Ok(())
                    }
                    "batched" => {
                        let mut invocation: Invocation<_> = serde_json::from_str(&text)?;

                        let arguments = invocation.arguments().unwrap();

                        let result = batched(arguments);

                        if let Some(id) = invocation.id() {
                            // result.forward(id.clone(), output).await?;
                            todo!()
                        }

                        Ok(())
                    }
                    "add_stream" => {
                        // let hub = Arc::clone(&self.hub);
                        // text_client_stream_invocation(
                        //     "add_stream",
                        //     &text,
                        //     move |_: EmptyArgs, b| {
                        //         // let result = async move { hub.add_stream(b).await };
                        //         // HubFutureWrapper(result)
                        //         todo!()
                        //     },
                        //     output,
                        //     Arc::clone(&self.client_streams_mapping),
                        // )
                        // .await
                        todo!()
                    }
                    _ => panic!(),
                }
            }
            MessageType::StreamInvocation => {
                let target: Target = serde_json::from_str(&text)?;
                match target.target.as_str() {
                    // "stream" => {
                    //     text_stream_invocation(
                    //         &text,
                    //         |args: StreamArgs| stream(args.0),
                    //         output,
                    //         Arc::clone(&self.ongoing_invocations),
                    //     )
                    //     .await
                    // }
                    // "stream_failure" => {
                    //     text_stream_invocation(
                    //         &text,
                    //         |args: StreamArgs| stream_failure(args.0),
                    //         output,
                    //         Arc::clone(&self.ongoing_invocations),
                    //     )
                    //     .await
                    // }
                    _ => panic!(),
                }
            }
            MessageType::StreamItem => {
                let message: Id = serde_json::from_str(&text)?;

                let mut guard = self.client_streams_mapping.lock().await;
                let cs = (*guard).get_mut(&message.invocation_id);

                if let Some(cs) = cs {
                    match cs.to_function.as_str() {
                        "add_stream" => text_stream_item::<i32>(&text, cs).await?,
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
}

// async fn text_invocation<'de, T, R, F, S>(
//     text: &'de str,
//     hub_function: F,
//     output: S,
// ) -> Result<(), Box<dyn std::error::Error>>
// where
//     T: Deserialize<'de>,
//     F: FnOnce(T) -> R,
//     R: HubResponse,
//     S: Sink<String> + Send + 'static + Unpin + Clone,
//     <S as Sink<String>>::Error: Debug + std::error::Error,
// {
//     let mut invocation: Invocation<T> = serde_json::from_str(text)?;

//     let arguments = invocation.arguments().unwrap();

//     let result = hub_function(arguments);

//     if let Some(id) = invocation.id() {
//         // result.forward(id.clone(), output).await?;
//         todo!()
//     }

//     Ok(())
// }

// async fn text_stream_invocation<'de, T, R, F, S>(
//     text: &'de str,
//     hub_function: F,
//     output: S,
//     invocations: Arc<Mutex<HashMap<String, tokio::task::JoinHandle<()>>>>,
// ) -> Result<(), Box<dyn std::error::Error>>
// where
//     T: Deserialize<'de>,
//     F: FnOnce(T) -> R,
//     R: HubResponse + 'static,
//     S: Sink<String> + Send + 'static + Unpin + Clone,
//     <S as Sink<String>>::Error: Debug + std::error::Error,
// {
//     let stream_invocation: StreamInvocation<T> = serde_json::from_str(text)?;

//     let arguments = stream_invocation.arguments.unwrap();
//     let invocation_id = stream_invocation.invocation_id;

//     // let result = hub_function(arguments).forward(invocation_id.clone(), output);

//     todo!();

//     // let invocation_id_clone = invocation_id.clone();
//     // let invocations_clone = Arc::clone(&invocations);
//     // let ongoing = tokio::spawn(async move {
//     //     result.await.unwrap();
//     //     let mut invocations = invocations_clone.lock().await;
//     //     (*invocations).remove(&invocation_id_clone);
//     // });

//     // let mut guard = invocations.lock().await;
//     // (*guard).insert(invocation_id, ongoing);

//     // Ok(())
// }

// async fn text_client_stream_invocation<'de, T, R, F, S, I>(
//     function_name: &'static str,
//     text: &'de str,
//     hub_function: F,
//     output: S,
//     client_streams_mapping: Arc<Mutex<HashMap<String, ClientStream>>>,
// ) -> Result<(), Box<dyn std::error::Error>>
// where
//     T: DeserializeOwned + Send + 'static,
//     F: FnOnce(T, RecvStream<'static, I>) -> R + Send + 'static,
//     I: 'static + Send,
//     R: HubResponse + Send + 'static,
//     S: Sink<String> + Send + 'static + Unpin + Clone,
//     <S as Sink<String>>::Error: Debug + std::error::Error,
// {
//     let mut invocation: Invocation<T> = serde_json::from_str(text)?;

//     let invocation_id = invocation.id().clone().unwrap();
//     let arguments = invocation.arguments().unwrap();

//     if let Some(e) = invocation.stream_ids {
//         let (tx, rx) = flume::bounded(100);

//         for incoming_stream in e {
//             let mut guard = client_streams_mapping.lock().await;
//             let cs = ClientStream {
//                 to_function: function_name.to_string(),
//                 sink: Box::new(tx.clone().into_sink()),
//             };
//             (*guard).insert(incoming_stream.clone(), cs);
//         }

//         let output_clone = output.clone();
//         let invocation_id_clone = invocation_id.clone();

//         tokio::spawn(async move {
//             let hub_function_future = hub_function(arguments, rx.into_stream());

//             // hub_function_future
//             //     .forward(invocation_id_clone, output_clone)
//             //     .await
//             //     .unwrap();
//         });
//     }

//     Ok(())
// }

async fn text_stream_item<'de, T>(
    text: &'de str,
    cs: &mut ClientStream,
) -> Result<(), Box<dyn std::error::Error>>
where
    T: Deserialize<'de> + 'static,
{
    let item: StreamItem<T> = serde_json::from_str(text)?;
    let sink = cs.sink.downcast_mut::<SendSink<T>>().unwrap();
    sink.send(item.item).await?;

    Ok(())
}

// ============= Extractors

#[derive(Deserialize, Debug)]
pub struct Args<T>(T);

// ============= Domain

pub fn non_blocking() {
    // nothing
}

pub fn identity(Args(a): Args<i32>) -> i32 {
    a
}

pub fn add(Args((a, b)): Args<(i32, i32)>) -> i32 {
    a + b
}

pub fn single_result_failure(Args((_, _)): Args<(u32, u32)>) -> Result<u32, String> {
    Err::<u32, String>("An error!".to_string())
}

pub fn batched(Args((count,)): Args<(usize,)>) -> Vec<usize> {
    std::iter::successors(Some(0usize), |p| Some(p + 1))
        .take(count)
        .collect::<Vec<usize>>()
}

pub fn stream(count: usize) -> impl HubResponse {
    HubStream::infallible(stream! {
        for i in 0..count {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            yield i;
        }
    })
}

pub fn stream_failure(count: usize) -> impl HubResponse {
    HubStream::fallible(stream! {
        for i in 0..count {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            yield Ok(i);
        }
        yield Err("Ran out of data!".to_string())
    })
}

pub async fn add_stream(input: impl Stream<Item = i32>) -> impl HubResponse {
    input.collect::<Vec<i32>>().await.into_iter().sum::<i32>()
}

#[derive(Deserialize, Clone, Debug)]
struct BatchedArgs(usize, #[serde(default)] ());

#[derive(Deserialize, Clone, Debug)]
struct SingleResultFailureArgs(u32, u32);

#[derive(Deserialize, Clone, Debug)]
struct StreamArgs(usize, #[serde(default)] ());

#[derive(Deserialize, Clone, Debug)]
struct EmptyArgs(#[serde(default)] (), #[serde(default)] ());
