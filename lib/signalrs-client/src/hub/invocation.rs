use super::error::HubError;
use crate::{
    messages::{ClientMessage, MessageEncoding},
    protocol::Arguments,
};
use serde_json::Value;
use thiserror::Error;

pub struct HubInvocation {
    pub(crate) message: ClientMessage,
    pub(crate) state: InvocationState,
}

pub struct InvocationState {
    pub(crate) arguments: ArgumentsLeft,
}

pub enum ArgumentsLeft {
    Text(std::vec::IntoIter<serde_json::Value>),
}

pub trait FromInvocation
where
    Self: Sized,
{
    fn try_from_invocation(request: &mut HubInvocation) -> Result<Self, ExtractionError>;
}

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

impl HubInvocation {
    pub(crate) fn new(message: ClientMessage) -> Result<Self, HubError> {
        let arguments = get_arguments(&message)?;

        Ok(HubInvocation {
            message,
            state: InvocationState { arguments },
        })
    }

    pub fn get_arguments(&mut self) -> &mut ArgumentsLeft {
        &mut self.state.arguments
    }
}

fn get_arguments(message: &ClientMessage) -> Result<ArgumentsLeft, ExtractionError> {
    match message.get_encoding() {
        MessageEncoding::Json => {
            let text = message.unwrap_text();
            let arguments: Arguments<Value> = serde_json::from_str(&text)?;
            match arguments.arguments.unwrap_or_default() {
                Value::Array(array) => Ok(ArgumentsLeft::Text(array.into_iter())),
                _ => Err(ExtractionError::NotAnArray),
            }
        }
    }
}
