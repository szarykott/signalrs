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

pub struct SignalRClient<Sink, Stream, Item> {
    sender: sender::SignalRClientSender<Sink>,
    receiver: receiver::SignalRClientReceiver<Stream, Item>,
}

pub fn new_text_client<Out, In>(output: Out, input: In) -> SignalRClient<Out, In, ClientMessage>
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

macro_rules! send_text_x {
    ($name:ident, $($ty:ident),+) => {
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

impl<Si, St> SignalRClient<Si, St, ClientMessage>
where
    Si: Sink<ClientMessage, Error = SignalRClientError> + Unpin + Clone,
    St: Stream<Item = ClientMessage> + Send + Unpin + 'static,
{
    pub async fn send_text_0(&mut self, target: impl ToString) -> Result<(), SignalRClientError> {
        self.sender.send_text0(target.to_string(), None).await
    }
    send_text_x!(send_text1, T1);
    send_text_x!(send_text2, T1, T2);
    send_text_x!(send_text3, T1, T2, T3);
    send_text_x!(send_text4, T1, T2, T3, T4);
    send_text_x!(send_text5, T1, T2, T3, T4, T5);
    send_text_x!(send_text6, T1, T2, T3, T4, T5, T6);
    send_text_x!(send_text7, T1, T2, T3, T4, T5, T6, T7);
    send_text_x!(send_text8, T1, T2, T3, T4, T5, T6, T7, T8);
    send_text_x!(send_text9, T1, T2, T3, T4, T5, T6, T7, T8, T9);
    send_text_x!(send_text10, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
    send_text_x!(send_text11, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
    send_text_x!(send_text12, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);
    send_text_x!(send_text13, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13);

    pub async fn invoke2<T1, T2, R>(
        &mut self,
        target: impl ToString,
        arg1: T1,
        arg2: T2,
    ) -> Result<R, SignalRClientError>
    where
        T1: IntoInvocationPart<T1> + Serialize + 'static,
        T2: IntoInvocationPart<T2> + Serialize + 'static,
        R: DeserializeOwned,
    {
        let invocation_id = Uuid::new_v4().to_string();

        let rx = self.receiver.setup_receive_once(invocation_id.clone());

        let result = self
            .sender
            .send_text2(target.to_string(), Some(invocation_id.clone()), arg1, arg2)
            .await;

        if let e @ Err(_) = result {
            self.receiver.remove_invocation(&invocation_id);
            e?;
        }

        let result = self.receiver.receive_once::<R>(invocation_id, rx).await?;

        let result = result.map_err(|x| SignalRClientError::ProtocolError {
            message: x.to_owned(), // FIXME: Error!
        });

        result
    }

    pub async fn invoke_stream1<T1, R>(
        &mut self,
        target: impl ToString,
        arg1: T1,
    ) -> Result<impl Stream<Item = Result<R, SignalRClientError>>, SignalRClientError>
    where
        T1: IntoInvocationPart<T1> + Serialize + 'static,
        R: DeserializeOwned + Send + 'static,
    {
        let invocation_id = Uuid::new_v4().to_string();

        let rx = self.receiver.setup_receive_stream(invocation_id.clone());

        let result = self
            .sender
            .send_text1(target.to_string(), Some(invocation_id.clone()), arg1)
            .await;

        if let e @ Err(_) = result {
            self.receiver.remove_invocation(&invocation_id);
            e?;
        }

        self.receiver.receive_stream::<R>(invocation_id, rx).await
    }
}
