use super::invocation::{ArgumentsLeft, ExtractionError, FromInvocation, HubInvocation};
use crate::{client::messages::MessageEncoding, protocol::Arguments};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::vec::IntoIter;

macro_rules! impl_from_invocation {
    ($($ty:ident),+) => {
        $(
            impl FromInvocation for $ty {
                fn try_from_invocation(request: &mut HubInvocation) -> Result<Self, ExtractionError> {
                    let remaining_arguments = match &mut request.state.arguments {
                        Some(arguments) => arguments,
                        None => get_arguments(request)?,
                    };

                    match remaining_arguments {
                        ArgumentsLeft::Text(iter) => from_text_value(iter),
                    }
                }
            }
        )+
    };
}

impl_from_invocation!(usize, isize);
impl_from_invocation!(f32, f64);
impl_from_invocation!(i8, i16, i32, i64, i128);
impl_from_invocation!(u8, u16, u32, u64, u128);
impl_from_invocation!(String);

fn get_arguments(request: &mut HubInvocation) -> Result<&mut ArgumentsLeft, ExtractionError> {
    let arguments = match request.message.get_encoding() {
        MessageEncoding::Json => {
            let text = request.message.unwrap_text();
            let arguments: Arguments<Value> = serde_json::from_str(&text)?;
            match arguments.arguments.unwrap_or_default() {
                Value::Array(array) => ArgumentsLeft::Text(array.into_iter()),
                _ => return Err(ExtractionError::NotAnArray),
            }
        }
    };
    Ok(request.state.arguments.get_or_insert(arguments))
}

fn from_text_value<T: DeserializeOwned>(args: &mut IntoIter<Value>) -> Result<T, ExtractionError> {
    let next = args.next().ok_or_else(|| ExtractionError::MissingArgs)?;
    serde_json::from_value(next).map_err(|e| e.into())
}
