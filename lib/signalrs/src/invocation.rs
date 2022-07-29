use crate::connection::ConnectionState;

pub struct HubInvocation {
    pub payload: Payload,
    pub connection_state: ConnectionState,
    pub invocation_state: InvocationState,
}

impl HubInvocation {
    pub(crate) fn text(payload: String, connection_state: ConnectionState) -> Self {
        HubInvocation {
            payload: Payload::Text(payload),
            connection_state,
            invocation_state: Default::default(),
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum Payload {
    Text(String),
    Binary(Vec<u8>),
}

#[derive(Default)]
pub struct InvocationState {
    pub next_stream_id_index: usize,
}

impl HubInvocation {
    pub fn unwrap_text(&self) -> String {
        match &self.payload {
            Payload::Text(v) => v.clone(),
            _ => unimplemented!(),
        }
    }
}
