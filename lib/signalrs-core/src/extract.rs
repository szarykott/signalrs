use serde::{de::DeserializeOwned, Deserialize};

use crate::{
    error::SignalRError,
    protocol::Arguments,
    request::{HubRequest, Payload},
};

pub trait FromRequest
where
    Self: Sized,
{
    fn try_from_request(request: &HubRequest) -> Result<Self, SignalRError>;
}

#[derive(Deserialize, Debug)]
pub struct Args<T>(pub T);

impl<T> FromRequest for Args<T>
where
    T: DeserializeOwned,
{
    fn try_from_request(request: &HubRequest) -> Result<Self, SignalRError> {
        match &request.payload {
            Payload::Text(text) => {
                let arguments: Arguments<T> = serde_json::from_str(text.as_str())?;

                if let Some(arguments) = arguments.arguments {
                    Ok(Args(arguments))
                } else {
                    Err(SignalRError::UnnspecifiedError)
                }
            }
            _ => unimplemented!(),
        }
    }
}
