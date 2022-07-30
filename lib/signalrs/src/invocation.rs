use tokio_util::sync::CancellationToken;

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
    pub(crate) next_stream_id_index: usize,
    pub(crate) invocation_id: Option<String>,
}

impl HubInvocation {
    pub fn unwrap_text(&self) -> String {
        match &self.payload {
            Payload::Text(v) => v.clone(),
            _ => unimplemented!(),
        }
    }

    pub fn get_cancellation_token(&self) -> Option<CancellationToken> {
        let invocation_id = match self.invocation_state.invocation_id {
            Some(ref id) => id,
            None => return None,
        };

        let existing_token = self
            .connection_state
            .inflight_invocations
            .get(invocation_id);

        if let Some(token) = existing_token {
            return Some(token);
        } else {
            let token = CancellationToken::new();

            self.connection_state
                .inflight_invocations
                .insert_token(invocation_id.to_string(), token.clone());

            return Some(token);
        }
    }
}
