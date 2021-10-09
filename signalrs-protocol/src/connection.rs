use crate::messages::Message;
use futures::{sink::Sink, stream::Stream};
use signalrs_error::SignalRError;

pub struct SignalRConnection<I, O> {
    state: State,
    input: I,
    output: O,
}

enum State {
    Uninitialized,
    ReceivedRequestHandshake,
    Established,
    InvocationSent,
    StreamInvocationSent
}

impl<I, O> SignalRConnection<I, O>
where
    I: Stream<Item = Message>,
    O: Sink<Message>,
{
    pub fn new(input: I, output: O) -> Self {
        SignalRConnection {
            state: State::Uninitialized,
            input,
            output,
        }
    }

    pub async fn send(&mut self, message: Message) -> Result<(), SignalRError> {
        match message {
            Message::HandshakeRequest(_, _) => if let State::Uninitialized = self.state {
                todo!()
            } else {
                Err(SignalRError::ProtocolViolation("HandshakeRequest only allowed on fresh connection".to_string()))
            },

            Message::HandshakeResponse(_, _) => if let State::ReceivedRequestHandshake = self.state {
                todo!()
            } else {
                Err(SignalRError::ProtocolViolation("HandshakeResponse only allowed just after receiving HandshakeRequest".to_string()))
            },

            Message::Close(_) => todo!(),
            Message::Invocation(_, _, _, _) => todo!(),
            Message::StreamInvocation(_, _, _, _) => todo!(),
            Message::StreamItem(_, _, _) => todo!(),
            Message::Completion(_, _, _, _) => todo!(),
            Message::CancelInvocation(_, _) => todo!(),
            Message::Ping => {
                // send ping
                todo!()
            },
        }
    }

    pub async fn receive(&mut self) -> Result<Message, SignalRError> {
        todo!()
    }
}
