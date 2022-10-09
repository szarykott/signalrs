mod error;
mod messages;
mod receiver;
mod sender;

use futures::{Sink, Stream};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

pub use self::{
    error::{ChannelSendError, SignalRClientError},
    messages::ClientMessage,
};
use self::{
    messages::MessageEncoding,
    receiver::SignalRClientReceiver,
    sender::{IntoInvocationPart, SignalRClientSender},
};

pub struct SignalRClient<Sink, Stream> {
    sender: sender::SignalRClientSender<Sink>,
    receiver: receiver::SignalRClientReceiver<Stream, ClientMessage>,
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
    stream_x!(invoke_stream9, send9, Arg1, Arg2, Arg3, Arg4, Arg5, Arg6, Arg7, Arg8, Arg9);
    stream_x!(invoke_stream10, send10, Arg1, Arg2, Arg3, Arg4, Arg5, Arg6, Arg7, Arg8, Arg9, Arg10);
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
}
