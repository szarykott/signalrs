use futures::{Stream, StreamExt};
use signalrs_core::{protocol::*, response::*};


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

// ======== Handshake

// impl HubInvoker {
//     #[allow(clippy::new_without_default)]
//     pub fn new() -> Self {
//         HubInvoker {
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
// }

// ========= Operation

// impl HubInvoker {
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

//         let RoutingData {
//             target,
//             message_type,
//         } = serde_json::from_str(&text)?;

//         match message_type {
//             MessageType::Invocation => {
//                 match target.unwrap_or("".to_string()).as_str() {
//                     "non_blocking" => {
//                         let invocation: Invocation<()> = serde_json::from_str(&text)?;

//                         let result = non_blocking();

//                         if let Some(id) = invocation.invocation_id {
//                             // result.forward(id.clone(), output).await?;
//                             todo!()
//                         }

//                         Ok(())
//                     }
//                     "add" => {
//                         let mut invocation: Invocation<_> = serde_json::from_str(&text)?;

//                         let arguments = invocation.arguments.unwrap();

//                         let result = add(arguments).into_stream();

//                         if let Some(id) = invocation.invocation_id {
//                             // result.forward(id.clone(), output).await?;
//                             todo!()
//                         }

//                         Ok(())
//                     }
//                     "single_result_failure" => {
//                         let mut invocation: Invocation<_> = serde_json::from_str(&text)?;

//                         let arguments = invocation.arguments.unwrap();

//                         let result = single_result_failure(arguments);

//                         if let Some(id) = invocation.invocation_id {
//                             // result.forward(id.clone(), output).await?;
//                             todo!()
//                         }

//                         Ok(())
//                     }
//                     "batched" => {
//                         let mut invocation: Invocation<_> = serde_json::from_str(&text)?;

//                         let arguments = invocation.arguments.unwrap();

//                         let result = batched(arguments);

//                         if let Some(id) = invocation.invocation_id {
//                             // result.forward(id.clone(), output).await?;
//                             todo!()
//                         }

//                         Ok(())
//                     }
//                     "add_stream" => {
//                         // let hub = Arc::clone(&self.hub);
//                         // text_client_stream_invocation(
//                         //     "add_stream",
//                         //     &text,
//                         //     move |_: EmptyArgs, b| {
//                         //         // let result = async move { hub.add_stream(b).await };
//                         //         // HubFutureWrapper(result)
//                         //         todo!()
//                         //     },
//                         //     output,
//                         //     Arc::clone(&self.client_streams_mapping),
//                         // )
//                         // .await
//                         todo!()
//                     }
//                     _ => panic!(),
//                 }
//             }
//             MessageType::StreamInvocation => {
//                 match target.unwrap_or_default().as_str() {
//                     // "stream" => {
//                     //     text_stream_invocation(
//                     //         &text,
//                     //         |args: StreamArgs| stream(args.0),
//                     //         output,
//                     //         Arc::clone(&self.ongoing_invocations),
//                     //     )
//                     //     .await
//                     // }
//                     // "stream_failure" => {
//                     //     text_stream_invocation(
//                     //         &text,
//                     //         |args: StreamArgs| stream_failure(args.0),
//                     //         output,
//                     //         Arc::clone(&self.ongoing_invocations),
//                     //     )
//                     //     .await
//                     // }
//                     _ => panic!(),
//                 }
//             }
//             MessageType::StreamItem => {
//                 let message: OptionalId = serde_json::from_str(&text)?;

//                 // let mut guard = self.client_streams_mapping.lock().await;
//                 // let cs = (*guard).get_mut(&message.invocation_id);

//                 // if let Some(cs) = cs {
//                 //     match cs.to_function.as_str() {
//                 //         "add_stream" => text_stream_item::<i32>(&text, cs).await?,
//                 //         _ => panic!(),
//                 //     }
//                 // }

//                 Ok(())
//             }
//             MessageType::Completion => {
//                 let message: OptionalId = serde_json::from_str(&text)?;

//                 // let mut guard = self.client_streams_mapping.lock().await;
//                 // let cs = (*guard).remove(&message.invocation_id);

//                 // if let Some(cs) = cs {
//                 //     drop(cs); // should terminate sender
//                 // }

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
// }

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

// async fn text_stream_item<'de, T>(
//     text: &'de str,
//     cs: &mut ClientStream,
// ) -> Result<(), Box<dyn std::error::Error>>
// where
//     T: Deserialize<'de> + 'static,
// {
//     let item: StreamItem<T> = serde_json::from_str(text)?;
//     let sink = cs.sink.downcast_mut::<SendSink<T>>().unwrap();
//     sink.send(item.item).await?;

//     Ok(())
// }

// ============= Extractors

// ============= Domain

// pub fn non_blocking() {
//     // nothing
// }

// pub async fn identity(Args(a): Args<i32>) -> i32 {
//     a
// }

// pub fn add(Args((a, b)): Args<(i32, i32)>) -> i32 {
//     a + b
// }

// pub fn single_result_failure(Args((_, _)): Args<(u32, u32)>) -> Result<u32, String> {
//     Err::<u32, String>("An error!".to_string())
// }

// pub fn batched(Args((count,)): Args<(usize,)>) -> Vec<usize> {
//     std::iter::successors(Some(0usize), |p| Some(p + 1))
//         .take(count)
//         .collect::<Vec<usize>>()
// }

// pub fn stream(count: usize) -> impl HubResponse {
//     HubStream::infallible(stream! {
//         for i in 0..count {
//             tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
//             yield i;
//         }
//     })
// }

// pub fn stream_failure(count: usize) -> impl HubResponse {
//     HubStream::fallible(stream! {
//         for i in 0..count {
//             tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
//             yield Ok(i);
//         }
//         yield Err("Ran out of data!".to_string())
//     })
// }

pub async fn add_stream(input: impl Stream<Item = i32>) -> impl HubResponse {
    input.collect::<Vec<i32>>().await.into_iter().sum::<i32>()
}
