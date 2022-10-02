mod error;
mod receiver;
mod sender;

use futures::{Sink, Stream};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

pub use self::error::{ChannelSendError, SignalRClientError};
use self::{
    receiver::SignalRClientReceiver,
    sender::{IntoInvocationPart, SignalRClientSender},
};

pub struct SignalRClient<Sink, Stream, Item> {
    sender: sender::SignalRClientSender<Sink>,
    receiver: receiver::SignalRClientReceiver<Stream, Item>,
}

pub fn new_text_client<Out, In>(output: Out, input: In) -> SignalRClient<Out, In, String>
where
    Out: Sink<String, Error = SignalRClientError> + Unpin + Clone,
    In: Stream<Item = String> + Send + Unpin + 'static,
{
    let mut receiver = SignalRClientReceiver {
        invocations: Arc::new(Mutex::new(HashMap::new())),
        incoming_messages: Some(input),
    };

    receiver.start_receiver_loop();

    SignalRClient {
        sender: SignalRClientSender { sink: output },
        receiver,
    }
}

impl<Si, St> SignalRClient<Si, St, String>
where
    Si: Sink<String, Error = SignalRClientError> + Unpin + Clone,
    St: Stream<Item = String> + Send + Unpin + 'static,
{
    pub async fn send2<T1, T2>(
        &mut self,
        target: impl ToString,
        arg1: T1,
        arg2: T2,
    ) -> Result<(), SignalRClientError>
    where
        T1: IntoInvocationPart<T1> + Serialize + 'static,
        T2: IntoInvocationPart<T2> + Serialize + 'static,
    {
        self.sender
            .send2(target.to_string(), None, arg1, arg2)
            .await
    }

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
            .send2(target.to_string(), Some(invocation_id.clone()), arg1, arg2)
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
            .send1(target.to_string(), Some(invocation_id.clone()), arg1)
            .await;

        if let e @ Err(_) = result {
            self.receiver.remove_invocation(&invocation_id);
            e?;
        }

        self.receiver.receive_stream::<R>(invocation_id, rx).await
    }
}
