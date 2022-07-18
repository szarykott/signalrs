// use async_stream::stream;
// use flume::r#async::{RecvStream, SendSink};
// use futures::{Sink, SinkExt, Stream, StreamExt};
// use serde;
// use serde::de::DeserializeOwned;
// use serde::Deserialize;
// use signalrs_core::{hub_response::*, protocol::*};
// use std::{any::Any, collections::HashMap, fmt::Debug, sync::Arc};
// use tokio;
// use tokio::sync::Mutex;

// const WEIRD_ENDING: &str = "\u{001E}";

// pub struct HubInvoker {
//     hub: Arc<Hub>,
//     ongoing_invocations: Arc<Mutex<HashMap<String, tokio::task::JoinHandle<()>>>>,
//     client_streams_mapping: Arc<Mutex<HashMap<String, ClientStream>>>,
// }

// pub struct ClientStream {
//     to_function: String,
//     sink: Box<dyn Any + Send>,
// }

// impl HubInvoker {
//     #[allow(clippy::new_without_default)]
//     pub fn new() -> Self {
//         HubInvoker {
//             hub: Arc::new(Hub {
//                 _counter: Arc::new(Mutex::new(0)),
//             }),
//             ongoing_invocations: Arc::new(Mutex::new(HashMap::new())),
//             client_streams_mapping: Arc::new(Mutex::new(HashMap::new())),
//         }
//     }

//     pub fn handshake(&self, input: &str) -> String {
//         let input = input.trim_end_matches(WEIRD_ENDING);

//         let request = serde_json::from_str::<HandshakeRequest>(input);

//         let response = match request {
//             Ok(request) => {
//                 if request.is_json() {
//                     HandshakeResponse::no_error()
//                 } else {
//                     HandshakeResponse::error("Unsupported protocol")
//                 }
//             }
//             Err(e) => HandshakeResponse::error(e),
//         };

//         match serde_json::to_string(&response) {
//             Ok(value) => format!("{}{}", value, WEIRD_ENDING),
//             Err(e) => e.to_string(),
//         }
//     }

//     pub async fn invoke_binary(&self, _data: &[u8]) -> Vec<u8> {
//         unimplemented!()
//     }

//     pub async fn invoke_text<T>(
//         &self,
//         text: String,
//         mut output: T,
//     ) -> Result<(), Box<dyn std::error::Error>>
//     where
//         T: Sink<String> + Send + 'static + Unpin + Clone,
//         <T as Sink<String>>::Error: Debug + std::error::Error,
//     {
//         let text = text.trim_end_matches(WEIRD_ENDING).to_owned();

//         match serde_json::from_str::<Type>(&text)?.message_type {
//             MessageType::Invocation => {
//                 let target: Target = serde_json::from_str(&text)?;
//                 match target.target.as_str() {
//                     "non_blocking" => {
//                         Self::text_invocation(&text, |_: EmptyArgs| self.hub.non_blocking(), output)
//                             .await
//                     }
//                     "add" => {
//                         Self::text_invocation(
//                             &text,
//                             |arguments: AddArgs| self.hub.add(arguments.0, arguments.1),
//                             output,
//                         )
//                         .await
//                     }
//                     "single_result_failure" => {
//                         Self::text_invocation(
//                             &text,
//                             |arguments: SingleResultFailureArgs| {
//                                 self.hub.single_result_failure(arguments.0, arguments.1)
//                             },
//                             output,
//                         )
//                         .await
//                     }
//                     "batched" => {
//                         Self::text_invocation(
//                             &text,
//                             |arguments: BatchedArgs| self.hub.batched(arguments.0),
//                             output,
//                         )
//                         .await
//                     }
//                     "add_stream" => {
//                         let hub = Arc::clone(&self.hub);
//                         Self::text_client_stream_invocation(
//                             "add_stream",
//                             &text,
//                             move |_: EmptyArgs, b| {
//                                 let result = async move { hub.add_stream(b).await };
//                                 HubFutureWrapper(result)
//                             },
//                             output,
//                             Arc::clone(&self.client_streams_mapping),
//                         )
//                         .await
//                     }
//                     _ => panic!(),
//                 }
//             }
//             MessageType::StreamInvocation => {
//                 let target: Target = serde_json::from_str(&text)?;
//                 match target.target.as_str() {
//                     "stream" => {
//                         Self::text_stream_invocation(
//                             &text,
//                             |args: StreamArgs| self.hub.stream(args.0),
//                             output,
//                             Arc::clone(&self.ongoing_invocations),
//                         )
//                         .await
//                     }
//                     "stream_failure" => {
//                         Self::text_stream_invocation(
//                             &text,
//                             |args: StreamArgs| self.hub.stream_failure(args.0),
//                             output,
//                             Arc::clone(&self.ongoing_invocations),
//                         )
//                         .await
//                     }
//                     _ => panic!(),
//                 }
//             }
//             MessageType::StreamItem => {
//                 let message: Id = serde_json::from_str(&text)?;

//                 let mut guard = self.client_streams_mapping.lock().await;
//                 let cs = (*guard).get_mut(&message.invocation_id);

//                 if let Some(cs) = cs {
//                     match cs.to_function.as_str() {
//                         "add_stream" => Self::text_stream_item::<i32>(&text, cs).await?,
//                         _ => panic!(),
//                     }
//                 }

//                 Ok(())
//             }
//             MessageType::Completion => {
//                 let message: Id = serde_json::from_str(&text)?;

//                 let mut guard = self.client_streams_mapping.lock().await;
//                 let cs = (*guard).remove(&message.invocation_id);

//                 if let Some(cs) = cs {
//                     drop(cs); // should terminate sender
//                 }

//                 Ok(())
//             }
//             MessageType::CancelInvocation => {
//                 let message: CancelInvocation = serde_json::from_str(&text)?;

//                 let mut guard = self.ongoing_invocations.lock().await;
//                 match (*guard).remove(&message.invocation_id) {
//                     Some(handle) => handle.abort(),
//                     None => { /* all good */ }
//                 };

//                 Ok(())
//             }
//             MessageType::Ping => {
//                 let ping = Ping::new();
//                 let s = serde_json::to_string(&ping)?;
//                 output.send(s + WEIRD_ENDING).await?;
//                 Ok(())
//             }
//             MessageType::Close => todo!(),
//             MessageType::Other => {
//                 /* panik or kalm? */
//                 todo!()
//             }
//         }
//     }

//     async fn text_invocation<'de, T, R, F, S>(
//         text: &'de str,
//         hub_function: F,
//         output: S,
//     ) -> Result<(), Box<dyn std::error::Error>>
//     where
//         T: Deserialize<'de>,
//         F: FnOnce(T) -> R,
//         R: HubResponse,
//         S: Sink<String> + Send + 'static + Unpin + Clone,
//         <S as Sink<String>>::Error: Debug + std::error::Error,
//     {
//         let mut invocation: Invocation<T> = serde_json::from_str(text)?;

//         let arguments = invocation.arguments().unwrap();

//         let result = hub_function(arguments);

//         if let Some(id) = invocation.id() {
//             // result.forward(id.clone(), output).await?;
//             todo!()
//         }

//         Ok(())
//     }

//     async fn text_stream_invocation<'de, T, R, F, S>(
//         text: &'de str,
//         hub_function: F,
//         output: S,
//         invocations: Arc<Mutex<HashMap<String, tokio::task::JoinHandle<()>>>>,
//     ) -> Result<(), Box<dyn std::error::Error>>
//     where
//         T: Deserialize<'de>,
//         F: FnOnce(T) -> R,
//         R: HubResponse + 'static,
//         S: Sink<String> + Send + 'static + Unpin + Clone,
//         <S as Sink<String>>::Error: Debug + std::error::Error,
//     {
//         let stream_invocation: StreamInvocation<T> = serde_json::from_str(text)?;

//         let arguments = stream_invocation.arguments.unwrap();
//         let invocation_id = stream_invocation.invocation_id;

//         todo!()

//         // let result = hub_function(arguments).forward(invocation_id.clone(), output);

//         // let invocation_id_clone = invocation_id.clone();
//         // let invocations_clone = Arc::clone(&invocations);
//         // let ongoing = tokio::spawn(async move {
//         //     result.await.unwrap();
//         //     let mut invocations = invocations_clone.lock().await;
//         //     (*invocations).remove(&invocation_id_clone);
//         // });

//         // let mut guard = invocations.lock().await;
//         // (*guard).insert(invocation_id, ongoing);

//         // Ok(())
//     }

//     async fn text_client_stream_invocation<'de, T, R, F, S, I>(
//         function_name: &'static str,
//         text: &'de str,
//         hub_function: F,
//         output: S,
//         client_streams_mapping: Arc<Mutex<HashMap<String, ClientStream>>>,
//     ) -> Result<(), Box<dyn std::error::Error>>
//     where
//         T: DeserializeOwned + Send + 'static,
//         F: FnOnce(T, RecvStream<'static, I>) -> R + Send + 'static,
//         I: 'static + Send,
//         R: HubResponse + Send + 'static,
//         S: Sink<String> + Send + 'static + Unpin + Clone,
//         <S as Sink<String>>::Error: Debug + std::error::Error,
//     {
//         let mut invocation: Invocation<T> = serde_json::from_str(text)?;

//         let invocation_id = invocation.id().clone().unwrap();
//         let arguments = invocation.arguments().unwrap();

//         if let Some(e) = invocation.stream_ids {
//             let (tx, rx) = flume::bounded(100);

//             for incoming_stream in e {
//                 let mut guard = client_streams_mapping.lock().await;
//                 let cs = ClientStream {
//                     to_function: function_name.to_string(),
//                     sink: Box::new(tx.clone().into_sink()),
//                 };
//                 (*guard).insert(incoming_stream.clone(), cs);
//             }

//             let output_clone = output.clone();
//             let invocation_id_clone = invocation_id.clone();

//             tokio::spawn(async move {
//                 let hub_function_future = hub_function(arguments, rx.into_stream());

//                 // hub_function_future
//                 //     .forward(invocation_id_clone, output_clone)
//                 //     .await
//                 //     .unwrap();
//             });
//         }

//         Ok(())
//     }

//     async fn text_stream_item<'de, T>(
//         text: &'de str,
//         cs: &mut ClientStream,
//     ) -> Result<(), Box<dyn std::error::Error>>
//     where
//         T: Deserialize<'de> + 'static,
//     {
//         let item: StreamItem<T> = serde_json::from_str(text)?;
//         let sink = cs.sink.downcast_mut::<SendSink<T>>().unwrap();
//         sink.send(item.item).await?;

//         Ok(())
//     }
// }

// pub struct Hub {
//     _counter: Arc<Mutex<usize>>,
// }

// impl Hub {
//     pub fn non_blocking(&self) {
//         // nothing
//     }

//     pub fn add(&self, a: i32, b: i32) -> impl HubResponse {
//         a + b
//     }

//     pub fn single_result_failure(&self, _a: u32, _b: u32) -> impl HubResponse {
//         Err::<u32, String>("An error!".to_string())
//     }

//     pub fn batched(&self, count: usize) -> impl HubResponse {
//         std::iter::successors(Some(0usize), |p| Some(p + 1))
//             .take(count)
//             .collect::<Vec<usize>>()
//     }

//     pub fn stream(&self, count: usize) -> impl HubResponse {
//         HubStream::infallible(stream! {
//             for i in 0..count {
//                 tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
//                 yield i;
//             }
//         })
//     }

//     pub fn stream_failure(&self, count: usize) -> impl HubResponse {
//         HubStream::fallible(stream! {
//             for i in 0..count {
//                 tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
//                 yield Ok(i);
//             }
//             yield Err("Ran out of data!".to_string())
//         })
//     }

//     pub async fn add_stream(&self, input: impl Stream<Item = i32>) -> impl HubResponse {
//         input.collect::<Vec<i32>>().await.into_iter().sum::<i32>()
//     }
// }

// #[derive(Deserialize, Clone, Debug)]
// struct BatchedArgs(usize, #[serde(default)] ());

// #[derive(Deserialize, Clone, Debug)]
// struct AddArgs(i32, i32);

// #[derive(Deserialize, Clone, Debug)]
// struct SingleResultFailureArgs(u32, u32);

// #[derive(Deserialize, Clone, Debug)]
// struct StreamArgs(usize, #[serde(default)] ());

// #[derive(Deserialize, Clone, Debug)]
// struct StreamFailureArgs(usize, #[serde(default)] ());

// #[derive(Deserialize, Clone, Debug)]
// struct EmptyArgs(#[serde(default)] (), #[serde(default)] ());
