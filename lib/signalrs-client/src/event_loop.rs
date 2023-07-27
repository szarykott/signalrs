use crate::protocol::MessageType;

struct HubConnection {
}



async fn event_loop(
    // 1. WebSocketHandle bidirectional 
    // - wrapper that produces and consumes signalR messages
    // - need to handle web socket specific messages (e.g WebSocket ping)
    web_socket: WebSocketHandle
    // 2. Client Hub - send only
) -> Result<EventLoopHandle, EventLoopError> // 3. returns handle that allows invoking methods and awaiting responses
{
    // setup event loop state and put in Arc<Mutex>
    loop {
        let serialized_signalr_messages = match web_socket.receive_signalr_message().await {
            Ok(message) => message.split(crate::messages::RECORD_SEPARATOR)
                .filter(|item| !item.trim().is_empty())
                .map(|item| serde_json::from_str(item))
                .collect::<Result<Vec<serde_json::Value>, _>>(),
            Err(error) => todo!(),
        };

        for message in serialized_signalr_messages.unwrap() {
            let message_type : MessageType = (message.get("type").and_then(|t| t.as_u64()).unwrap() as u8).into();
            
            match message_type {
                MessageType::Invocation => todo!(),
                MessageType::StreamItem => todo!(),
                MessageType::Completion => todo!(),
                MessageType::StreamInvocation => todo!(),
                MessageType::CancelInvocation => todo!(),
                MessageType::Ping => todo!(),
                MessageType::Close => todo!(),
                MessageType::Other => todo!(),
            }


        }
    }
    // spawn the loop in tokio
    Ok(EventLoopHandle::new())
}

struct WebSocketHandle;

impl WebSocketHandle {
    async fn receive_signalr_message(&self) -> Result<String, String> { panic!() }
    async fn send_signalr_message(&self, message: String) -> Result<(), String> { panic!() }
}

struct EventLoopHandle {

}

impl EventLoopHandle {
    pub fn new() -> EventLoopHandle {
        EventLoopHandle {  }
    }
}

enum EventLoopError {

}