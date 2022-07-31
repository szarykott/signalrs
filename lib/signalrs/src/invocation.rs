use tokio_util::sync::CancellationToken;

use crate::{
    connection::ConnectionState, error::SignalRError, protocol::OptionalId, response::ResponseSink,
};

pub struct HubInvocation {
    pub(crate) payload: Payload,
    pub(crate) connection_state: ConnectionState,
    pub(crate) invocation_state: InvocationState,
    pub(crate) output: ResponseSink,
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

        invocation.invocation_state.invocation_id = invocation_id;

        Ok(invocation)
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
