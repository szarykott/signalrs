use flume::r#async::{RecvStream, SendSink};
use futures::{Future, Sink, SinkExt};
use serde;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use signalrs_core::{hub_response::*, protocol::*};
use signalrs_macros::signalr_fn;
use std::{
    any::Any,
    collections::{HashMap, HashSet},
    fmt::Debug,
    pin::Pin,
    sync::Arc,
};
use tokio;
use tokio::sync::Mutex;

const WEIRD_ENDING: &str = "\u{001E}";

pub struct HubInvoker<Hub, Out> {
    hub: HubDescriptor<Hub, Out>,
    ongoing_invocations: Arc<Mutex<HashMap<String, tokio::task::JoinHandle<()>>>>,
    client_streams_mapping: Arc<Mutex<HashMap<String, ClientStream>>>,
}

pub struct ClientStream {
    to_function: String,
    sink: Box<dyn Any + Send>,
}

impl<Hub, Out> HubInvoker<Hub, Out>
where
    Out: Sink<String> + Send + 'static + Unpin + Clone,
    <Out as Sink<String>>::Error: Debug + std::error::Error,
{
    pub fn new(state: HubDescriptor<Hub, Out>) -> Self {
        HubInvoker {
            hub: state,
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

    pub async fn invoke_text(
        &self,
        text: String,
        mut output: Out,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let text = text.trim_end_matches(WEIRD_ENDING).to_owned();

        match serde_json::from_str::<Type>(&text)?.message_type {
            MessageType::Invocation | MessageType::StreamInvocation => {
                let target = serde_json::from_str::<Target>(&text)?.target;
                if let Some(method) = self.hub.methods.get(&target) {
                    let hub = Arc::clone(&self.hub.hub);
                    (method.action)(hub, text, output).await?;
                }

                Ok(())
            }
            MessageType::StreamItem => {
                let message: Id = serde_json::from_str(&text)?;

                let mut guard = self.client_streams_mapping.lock().await;
                let cs = (*guard).get_mut(&message.invocation_id);

                if let Some(cs) = cs {
                    if let Some(method) = self.hub.methods.get(&cs.to_function) {
                        let hub = Arc::clone(&self.hub.hub);
                        (method.action)(hub, text, output).await?;
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

pub struct HubDescriptor<Hub, Out> {
    hub: Arc<Hub>,
    methods: HashMap<String, MethodDescriptor<Hub, Out>>,
}

pub struct MethodDescriptor<Hub, Out> {
    action: Box<
        dyn Fn(
            Arc<Hub>,
            String,
            Out,
        ) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error>>>>>,
    >,
}

pub struct DaHub;

impl DaHub {
    #[signalr_fn]
    pub fn do_it(&self, arg: i32) -> impl HubResponse {
        arg * arg
    }
}

pub fn do_it_descriptor<Out>() -> MethodDescriptor<DaHub, Out>
where
    Out: Sink<String> + Send + 'static + Unpin + Clone,
    <Out as Sink<String>>::Error: Debug + std::error::Error,
{
    // MethodDescriptor {
    //     action: Box::new(|hub, text, output| {
    //         Box::pin(text_invocation(text, move |arg| hub.do_it(arg), output))
    //     }),
    // }

    todo!()
}

async fn text_invocation<T, R, F, S>(
    text: String,
    hub_function: F,
    output: S,
) -> Result<(), Box<dyn std::error::Error>>
where
    T: DeserializeOwned,
    F: FnOnce(T) -> R,
    R: HubResponse,
    S: Sink<String> + Send + 'static + Unpin + Clone,
    <S as Sink<String>>::Error: Debug + std::error::Error,
{
    let mut invocation: Invocation<T> = serde_json::from_str(text.as_str())?;

    let arguments = invocation.arguments().unwrap();

    let result = hub_function(arguments);

    if let Some(id) = invocation.id() {
        result.forward(id.clone(), output).await?;
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
    let stream_invocation: StreamInvocation<T> = serde_json::from_str(text)?;

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
    text: &'de str,
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
    let mut invocation: Invocation<T> = serde_json::from_str(text)?;

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
    let item: StreamItem<T> = serde_json::from_str(text)?;
    let sink = cs.sink.downcast_mut::<SendSink<T>>().unwrap();
    sink.send(item.item).await?;

    Ok(())
}
