use super::invocation::{ArgumentsLeft, ExtractionError, FromInvocation, HubInvocation};
use futures::Stream;
use serde::de::DeserializeOwned;

/// Stream of data from server to client
///
/// Can only appear as an argument to a hub method.
/// It will be extracted from HubInvocation and automagically streamed.
struct StreamArg<T>(Box<dyn Stream<Item = T>>);

/// Marker trait used to trick Rust orphan implementations system
///
/// And preserve a few other options
pub trait HubArgument {}

impl<T> FromInvocation for T
where
    T: HubArgument + DeserializeOwned,
{
    fn try_from_invocation(request: &mut HubInvocation) -> Result<Self, ExtractionError> {
        match &mut request.state.arguments {
            ArgumentsLeft::Text(args) => {
                let next = args.next().ok_or_else(|| ExtractionError::MissingArgs)?;
                serde_json::from_value(next).map_err(|e| e.into())
            }
        }
    }
}

impl<T> FromInvocation for StreamArg<T> {
    fn try_from_invocation(_request: &mut HubInvocation) -> Result<Self, ExtractionError> {
        // no body yet, here to ensure that Rust will not detect conflicting implementation in some cases
        unimplemented!("not yet implemented")
    }
}

macro_rules! impl_hub_argument {
    ($($ty:ident),+) => {
        $(
            impl HubArgument for $ty {}
        )+
    };
}

impl_hub_argument!(usize, isize);
impl_hub_argument!(f32, f64);
impl_hub_argument!(i8, i16, i32, i64, i128);
impl_hub_argument!(u8, u16, u32, u64, u128);
impl_hub_argument!(String);
