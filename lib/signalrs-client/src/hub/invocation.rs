use thiserror::Error;

use crate::messages::ClientMessage;

pub struct HubInvocation {
    pub(crate) message: ClientMessage,
    pub(crate) state: InvocationState,
}

#[derive(Default)]
pub struct InvocationState {
    pub(crate) arguments: Option<ArgumentsLeft>,
}

pub enum ArgumentsLeft {
    Text(std::vec::IntoIter<serde_json::Value>),
}

impl HubInvocation {
    pub fn new(message: ClientMessage) -> Self {
        HubInvocation {
            message,
            state: Default::default(),
        }
    }
}

pub trait FromInvocation
where
    Self: Sized,
{
    fn try_from_invocation(request: &mut HubInvocation) -> Result<Self, ExtractionError>;
}
// ============= Error

#[derive(Debug, Error)]
pub enum ExtractionError {
    #[error("Arguments not provided in the invocation")]
    MissingArgs,
    #[error("Stream not provided in the invocation")]
    MissingStreamIds,
    #[error("Number of requested client streams exceeds the number of streams in the invocation")]
    NotEnoughStreamIds,
    #[error("JSON deserialization error")]
    JsonError {
        #[from]
        source: serde_json::Error,
    },
    #[error("Provided arguemnts were not JSON array")]
    NotAnArray,
    #[error("An error occured : {0}")]
    UserDefined(String),
}
