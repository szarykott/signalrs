use std::collections::HashMap;

type InvocationId = String;
type Target = String;
type Arguments = Vec<String>;
type Error = String;
type Result = String;
type Item = String;
type Headers = Option<HashMap<String, String>>;

/// SignalR message
pub enum Message {
    /// Sent by the client to agree on the message format.
    HandshakeRequest(Headers, MessageFormat),
    /// Sent by the server as an acknowledgment of the previous `HandshakeRequest` message. Contains an error if the handshake failed.
    HandshakeResponse(Headers, Option<Error>),
    /// Sent by the server when a connection is closed. Contains an error if the connection was closed because of an error.
    Close(Headers),
    /// Indicates a request to invoke a particular method (the Target) with provided Arguments on the remote endpoint.
    Invocation(Headers, Option<InvocationId>, Target, Arguments),
    /// Indicates a request to invoke a streaming method (the Target) with provided Arguments on the remote endpoint.
    StreamInvocation(Headers, InvocationId, Target, Arguments),
    /// Indicates individual items of streamed response data from a previous `StreamInvocation` message.
    StreamItem(Headers, InvocationId, Item),
    /// Indicates a previous Invocation or StreamInvocation has completed.
    /// Contains an error if the invocation concluded with an error or the result of a non-streaming method invocation.
    /// The result will be absent for void methods.
    /// In case of streaming invocations no further StreamItem messages will be received.
    Completion(Headers, InvocationId, Option<Result>, Option<Error>),
    /// Sent by the client to cancel a streaming invocation on the server.
    CancelInvocation(Headers, InvocationId),
    /// Sent by either party to check if the connection is active.
    Ping,
}

/// Message format used during SignalR exchange
pub enum MessageFormat {
    /// A JSON format
    Json,
    /// A MessagePack format
    MessagePack,
}

const CLOSE: u16 = 7;

impl Message {
    fn get_type(&self) -> u16 {
        match self {
            Message::HandshakeRequest(_, _) => 0,
            Message::HandshakeResponse(_, _) => 0,
            Message::Close(_) => CLOSE,
            Message::Invocation(_, _, _, _) => 1,
            Message::StreamInvocation(_, _, _, _) => 4,
            Message::StreamItem(_, _, _) => 2,
            Message::Completion(_, _, _, _) => 3,
            Message::CancelInvocation(_, _) => 5,
            Message::Ping => 6,
        }
    }
}
