use crate::{
    connection::ConnectionState, error::SignalRError, protocol::OptionalId, response::ResponseSink,
};
use tokio_util::sync::CancellationToken;

pub struct HubInvocation {
    pub(crate) payload: Payload,
    pub(crate) connection_state: ConnectionState,
    pub(crate) invocation_state: InvocationState,
    pub(crate) output: ResponseSink,
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
    pub(crate) fn text(
        payload: String,
        connection_state: ConnectionState,
        output: ResponseSink,
    ) -> Result<Self, SignalRError> {
        let OptionalId { invocation_id } = serde_json::from_str(&payload)?;

        let mut invocation = HubInvocation {
            payload: Payload::Text(payload),
            connection_state,
            invocation_state: Default::default(),
            output,
        };

        if let Some(id) = &invocation_id {
            invocation.set_cancellation_token(id);
        }
        invocation.invocation_state.invocation_id = invocation_id;

        Ok(invocation)
    }

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
            return Some(self.set_cancellation_token(invocation_id));
        }
    }

    fn set_cancellation_token(&self, invocation_id: &str) -> CancellationToken {
        let token = CancellationToken::new();

        self.connection_state
            .inflight_invocations
            .insert_token(invocation_id.to_string(), token.clone());

        token
    }
}

impl Drop for HubInvocation {
    fn drop(&mut self) {
        if let Some(id) = &self.invocation_state.invocation_id {
            self.connection_state.inflight_invocations.remove(&id);
        }
    }
}
