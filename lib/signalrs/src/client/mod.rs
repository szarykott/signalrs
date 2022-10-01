mod error;
mod receiver;
mod sender;

use futures::{Sink, Stream};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

pub use self::error::{ChannelSendError, SignalRClientError};
use self::{receiver::SignalRClientReceiver, sender::SignalRClientSender};

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
