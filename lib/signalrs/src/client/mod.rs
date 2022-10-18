mod error;
mod messages;
mod receiver;
mod sender;

use futures::{Sink, Stream, StreamExt};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    clone,
    collections::HashMap,
    marker::PhantomData,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

use crate::protocol::{Invocation, StreamInvocation};

pub use self::{
    error::{ChannelSendError, SignalRClientError},
    messages::ClientMessage,
};
use self::{
    messages::MessageEncoding,
    receiver::SignalRClientReceiver,
    sender::{IntoInvocationPart, InvocationPart, SignalRClientSender},
};

pub struct SignalRClient<Sink, Stream> {
    sender: sender::SignalRClientSender<Sink>,
    receiver: receiver::SignalRClientReceiver<Stream>,
    encoding: MessageEncoding,
}

pub fn new_text_client<Out, In>(output: Out, input: In) -> SignalRClient<Out, In>
where
    Out: Sink<ClientMessage, Error = SignalRClientError> + Unpin + Clone,
    In: Stream<Item = ClientMessage> + Send + Unpin + 'static,
{
    let mut receiver = SignalRClientReceiver {
        invocations: Arc::new(Mutex::new(HashMap::new())),
        incoming_messages: Some(input),
        encoding: MessageEncoding::Json,
    };

    receiver.start_receiver_loop();

    SignalRClient {
        sender: SignalRClientSender {
            sink: output,
            encoding: MessageEncoding::Json,
        },
        receiver,
        encoding: MessageEncoding::Json,
    }
}

macro_rules! send_x {
    ($name:ident, $($ty:ident),+) => {
        /// Invokes a method without requesting a response
        #[allow(non_snake_case)]
        pub async fn $name<$($ty,)+>(
            &mut self,
            target: impl ToString,
            $(
                $ty: $ty,
            )+
        ) -> Result<(), SignalRClientError>
        where
            $(
                $ty: IntoInvocationPart<$ty> + Serialize + 'static,
            )+
        {
            self.sender
                .$name(target.to_string(), None, $($ty,)+)
                .await
        }
    };
}

macro_rules! invoke_x {
    ($name:ident, $call_func:ident, $($ty:ident),+) => {
        #[allow(non_snake_case)]
        pub async fn $name<$($ty,)+ R>(
            &mut self,
            target: impl ToString,
            $(
                $ty: $ty,
            )+
        ) -> Result<R, SignalRClientError>
        where
            $(
                $ty: IntoInvocationPart<$ty> + Serialize + 'static,
            )+
            R: DeserializeOwned,
        {
            let invocation_id = Uuid::new_v4().to_string();

            let rx = self.receiver.setup_receive_once(&invocation_id);

            let send_result = self
                .sender
                .$call_func(target.to_string(), Some(invocation_id.clone()), $($ty,)+)
                .await;

            if let e @ Err(_) = send_result {
                self.receiver.remove_invocation(&invocation_id);
                e?;
            }

            self.receiver
                .receive_once::<R>(invocation_id, rx)
                .await?
                .map_err(|x| SignalRClientError::ProtocolError {
                    message: x.to_owned(), // FIXME: Error!
                })
        }
    };
}

macro_rules! stream_x {
    ($name:ident, $call_func:ident, $($ty:ident),+) => {
        #[allow(non_snake_case)]
        pub async fn $name<$($ty,)+ R>(
            &mut self,
            target: impl ToString,
            $(
                $ty: $ty,
            )+
        ) -> Result<impl Stream<Item = Result<R, SignalRClientError>>, SignalRClientError>
        where
            $(
                $ty: IntoInvocationPart<$ty> + Serialize + 'static,
            )+
            R: DeserializeOwned + Send + 'static,
        {
            let invocation_id = Uuid::new_v4().to_string();

            let rx = self.receiver.setup_receive_stream(invocation_id.clone());

            let result = self
                .sender
                .$call_func(target.to_string(), Some(invocation_id.clone()), $($ty,)+)
                .await;

            if let e @ Err(_) = result {
                self.receiver.remove_invocation(&invocation_id);
                e?;
            }

            self.receiver.receive_stream::<R>(invocation_id, rx).await
        }
    };
}

impl<Si, St> SignalRClient<Si, St>
where
    Si: Sink<ClientMessage, Error = SignalRClientError> + Unpin + Clone,
    St: Stream<Item = ClientMessage> + Send + Unpin + 'static,
{
    /// Invokes a method without requesting a response
    pub async fn send0(&mut self, target: impl ToString) -> Result<(), SignalRClientError> {
        self.sender.send0(target.to_string(), None).await
    }
    send_x!(send1, Arg1);
    send_x!(send2, Arg1, Arg2);
    send_x!(send3, Arg1, Arg2, Arg3);
    send_x!(send4, Arg1, Arg2, Arg3, Arg4);
    send_x!(send5, Arg1, Arg2, Arg3, Arg4, Arg5);
    send_x!(send6, Arg1, Arg2, Arg3, Arg4, Arg5, Arg6);
    send_x!(send7, Arg1, Arg2, Arg3, Arg4, Arg5, Arg6, Arg7);
    send_x!(send8, Arg1, Arg2, Arg3, Arg4, Arg5, Arg6, Arg7, Arg8);
    send_x!(send9, Arg1, Arg2, Arg3, Arg4, Arg5, Arg6, Arg7, Arg8, Arg9);
    send_x!(send10, Arg1, Arg2, Arg3, Arg4, Arg5, Arg6, Arg7, Arg8, Arg9, Arg10);
    send_x!(send11, Arg1, Arg2, Arg3, Arg4, Arg5, Arg6, Arg7, Arg8, Arg9, Arg10, Arg11);
    send_x!(send12, Arg1, Arg2, Arg3, Arg4, Arg5, Arg6, Arg7, Arg8, Arg9, Arg10, Arg11, Arg12);
    send_x!(
        send13, Arg1, Arg2, Arg3, Arg4, Arg5, Arg6, Arg7, Arg8, Arg9, Arg10, Arg11, Arg12, Arg13
    );

    pub async fn invoke0<R>(&mut self, target: impl ToString) -> Result<R, SignalRClientError>
    where
        R: DeserializeOwned,
    {
        let invocation_id = Uuid::new_v4().to_string();

        let rx = self.receiver.setup_receive_once(&invocation_id);

        let send_result = self
            .sender
            .send0(target.to_string(), Some(invocation_id.clone()))
            .await;

        if let e @ Err(_) = send_result {
            self.receiver.remove_invocation(&invocation_id);
            e?;
        }

        self.receiver
            .receive_once::<R>(invocation_id, rx)
            .await?
            .map_err(|x| SignalRClientError::ProtocolError {
                message: x.to_owned(), // FIXME: Error!
            })
    }
    invoke_x!(invoke1, send1, Arg1);
    invoke_x!(invoke2, send2, Arg1, Arg2);
    invoke_x!(invoke3, send3, Arg1, Arg2, Arg3);
    invoke_x!(invoke4, send4, Arg1, Arg2, Arg3, Arg4);
    invoke_x!(invoke5, send5, Arg1, Arg2, Arg3, Arg4, Arg5);
    invoke_x!(invoke6, send6, Arg1, Arg2, Arg3, Arg4, Arg5, Arg6);
    invoke_x!(invoke7, send7, Arg1, Arg2, Arg3, Arg4, Arg5, Arg6, Arg7);
    invoke_x!(invoke8, send8, Arg1, Arg2, Arg3, Arg4, Arg5, Arg6, Arg7, Arg8);
    invoke_x!(invoke9, send9, Arg1, Arg2, Arg3, Arg4, Arg5, Arg6, Arg7, Arg8, Arg9);
    invoke_x!(invoke10, send10, Arg1, Arg2, Arg3, Arg4, Arg5, Arg6, Arg7, Arg8, Arg9, Arg10);
    invoke_x!(invoke11, send11, Arg1, Arg2, Arg3, Arg4, Arg5, Arg6, Arg7, Arg8, Arg9, Arg10, Arg11);
    invoke_x!(
        invoke12, send12, Arg1, Arg2, Arg3, Arg4, Arg5, Arg6, Arg7, Arg8, Arg9, Arg10, Arg11, Arg12
    );
    invoke_x!(
        invoke13, send13, Arg1, Arg2, Arg3, Arg4, Arg5, Arg6, Arg7, Arg8, Arg9, Arg10, Arg11,
        Arg12, Arg13
    );

    pub async fn invoke_stream0<Arg1, R>(
        &mut self,
        target: impl ToString,
    ) -> Result<impl Stream<Item = Result<R, SignalRClientError>>, SignalRClientError>
    where
        Arg1: IntoInvocationPart<Arg1> + Serialize + 'static,
        R: DeserializeOwned + Send + 'static,
    {
        let invocation_id = Uuid::new_v4().to_string();

        let rx = self.receiver.setup_receive_stream(invocation_id.clone());

        let result = self
            .sender
            .send0(target.to_string(), Some(invocation_id.clone()))
            .await;

        if let e @ Err(_) = result {
            self.receiver.remove_invocation(&invocation_id);
            e?;
        }

        self.receiver.receive_stream::<R>(invocation_id, rx).await
    }
    stream_x!(invoke_stream1, send1, Arg1);
    stream_x!(invoke_stream2, send2, Arg1, Arg2);
    stream_x!(invoke_stream3, send3, Arg1, Arg2, Arg3);
    stream_x!(invoke_stream4, send4, Arg1, Arg2, Arg3, Arg4);
    stream_x!(invoke_stream5, send5, Arg1, Arg2, Arg3, Arg4, Arg5);
    stream_x!(invoke_stream6, send6, Arg1, Arg2, Arg3, Arg4, Arg5, Arg6);
    stream_x!(invoke_stream7, send7, Arg1, Arg2, Arg3, Arg4, Arg5, Arg6, Arg7);
    stream_x!(invoke_stream8, send8, Arg1, Arg2, Arg3, Arg4, Arg5, Arg6, Arg7, Arg8);
    stream_x!(
        invoke_stream9,
        send9,
        Arg1,
        Arg2,
        Arg3,
        Arg4,
        Arg5,
        Arg6,
        Arg7,
        Arg8,
        Arg9
    );
    stream_x!(
        invoke_stream10,
        send10,
        Arg1,
        Arg2,
        Arg3,
        Arg4,
        Arg5,
        Arg6,
        Arg7,
        Arg8,
        Arg9,
        Arg10
    );
    stream_x!(
        invoke_stream11,
        send11,
        Arg1,
        Arg2,
        Arg3,
        Arg4,
        Arg5,
        Arg6,
        Arg7,
        Arg8,
        Arg9,
        Arg10,
        Arg11
    );
    stream_x!(
        invoke_stream12,
        send12,
        Arg1,
        Arg2,
        Arg3,
        Arg4,
        Arg5,
        Arg6,
        Arg7,
        Arg8,
        Arg9,
        Arg10,
        Arg11,
        Arg12
    );
    stream_x!(
        invoke_stream13,
        send13,
        Arg1,
        Arg2,
        Arg3,
        Arg4,
        Arg5,
        Arg6,
        Arg7,
        Arg8,
        Arg9,
        Arg10,
        Arg11,
        Arg12,
        Arg13
    );

    /// Creates method invocation builder with a specified named hub method
    pub fn method<'a>(
        &'a mut self,
        target: impl ToString,
    ) -> SignalRSendBuilder<'a, Si, St, AcceptingArgs> {
        SignalRSendBuilder {
            sender: &mut self.sender,
            receiver: &mut self.receiver,
            encoding: self.encoding,
            state: AcceptingArgs {
                method: target.to_string(),
                arguments: Default::default(),
                streams: Default::default(),
            },
        }
    }
}

pub struct SignalRSendBuilder<'a, Si, St, T> {
    sender: &'a mut sender::SignalRClientSender<Si>,
    receiver: &'a mut receiver::SignalRClientReceiver<St>,
    encoding: MessageEncoding,
    state: T,
}

pub struct AcceptingArgs {
    method: String,
    arguments: Vec<serde_json::Value>,
    streams: Vec<Box<dyn Stream<Item = Result<ClientMessage, SignalRClientError>> + Unpin>>,
}

impl<'a, Si, St> SignalRSendBuilder<'a, Si, St, AcceptingArgs>
where
    Si: Sink<ClientMessage, Error = SignalRClientError> + Unpin + Clone,
    St: Stream<Item = ClientMessage> + Send + Unpin + 'static,
{
    /// Adds ordered argument to invocation
    pub fn arg<A>(&mut self, arg: A) -> Result<&mut Self, SignalRClientError>
    where
        A: IntoInvocationPart<A> + Serialize + 'static,
    {
        match arg.into() {
            InvocationPart::Argument(arg) => self.state.arguments.push(serde_json::to_value(arg)?),
            InvocationPart::Stream(stream) => {
                let encoding = self.encoding;
                self.state.streams.push(Box::new(
                    stream.map(move |x| encoding.serialize(x).map_err(|x| x.into())),
                )
                    as Box<dyn Stream<Item = Result<ClientMessage, SignalRClientError>> + Unpin>);
            }
        };

        Ok(self)
    }

    /// Invokes a hub method on the server without waiting for a response
    pub async fn send(self) -> Result<(), SignalRClientError> {
        let mut invocation = Invocation::new_non_blocking(
            self.state.method,
            Self::args_as_option(self.state.arguments),
        );

        let stream_ids = Self::get_stream_ids(self.state.streams.len());
        invocation.with_streams(stream_ids.clone());

        let serialized = self.encoding.serialize(&invocation)?;

        self.sender
            .actually_send2(serialized, stream_ids.into_iter().zip(self.state.streams).collect())
            .await
    }

    /// Invokes a hub method on the server expecting a single response
    pub async fn invoke<T>(self) -> Result<T, SignalRClientError>
    where
        T: DeserializeOwned,
    {
        let mut invocation = Invocation::new_non_blocking(
            self.state.method,
            Self::args_as_option(self.state.arguments),
        );

        let invocation_id = Uuid::new_v4().to_string();
        invocation.add_invocation_id(invocation_id.clone());

        let stream_ids = Self::get_stream_ids(self.state.streams.len());
        invocation.with_streams(stream_ids.clone());

        let serialized = self.encoding.serialize(&invocation)?;

        let rx = self.receiver.setup_receive_once(&invocation_id);

        let send_result = self
            .sender
            .actually_send2(serialized, stream_ids.into_iter().zip(self.state.streams).collect())
            .await;

        if let e @ Err(_) = send_result {
            self.receiver.remove_invocation(&invocation_id);
            e?;
        }

        self.receiver
            .receive_once::<T>(invocation_id, rx)
            .await?
            .map_err(|x| SignalRClientError::ProtocolError {
                message: x.to_owned(), // FIXME: Error!
            })
    }

    /// Invokes a hub method on the server expecting a stream response
    pub async fn invoke_stream<T>(
        self,
    ) -> Result<impl Stream<Item = Result<T, SignalRClientError>>, SignalRClientError>
    where
        T: DeserializeOwned + Send + 'static,
    {
        let invocation_id = Uuid::new_v4().to_string();

        let mut invocation = StreamInvocation::new(
            invocation_id.clone(),
            self.state.method,
            Self::args_as_option(self.state.arguments),
        );

        let stream_ids = Self::get_stream_ids(self.state.streams.len());
        invocation.with_streams(stream_ids.clone());

        let serialized = self.encoding.serialize(&invocation)?;

        let rx = self.receiver.setup_receive_stream(invocation_id.clone());

        let result = self
            .sender
            .actually_send2(serialized, stream_ids.into_iter().zip(self.state.streams).collect())
            .await;

        if let e @ Err(_) = result {
            self.receiver.remove_invocation(&invocation_id);
            e?;
        }

        self.receiver.receive_stream::<T>(invocation_id, rx).await
    }

    fn args_as_option(arguments: Vec<serde_json::Value>) -> Option<Vec<serde_json::Value>> {
        if arguments.is_empty() {
            None
        } else {
            Some(arguments)
        }
    }

    fn get_stream_ids(num_streams: usize) -> Vec<String> {
        let mut stream_ids = Vec::new();
        if num_streams > 0 {
            for _ in 0..num_streams {
                stream_ids.push(Uuid::new_v4().to_string());
            }
        }

        stream_ids
    }
}
