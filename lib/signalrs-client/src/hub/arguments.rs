use super::invocation::{ArgumentsLeft, ExtractionError, FromInvocation, HubInvocation};
use futures::Stream;
use serde::de::DeserializeOwned;

/// Stream of data from server to client
///
/// Can only appear as an argument to a hub method.
/// It will be extracted from HubInvocation and automagically streamed.
struct StreamArg<T>(Box<dyn Stream<Item = T>>);

/// Represets a [`Hub`](crate::hub::Hub) method's argument.
///
/// Meant for all user defined types, as implementation for primitives should be provided out of the box.
/// Should be derived alongside [`Deserialize`](serde::Deserialize) using [`HubArgument`](signalrs_derive::HubArgument) derive macro.
///
/// # Example
/// ```rust,no_run
/// use serde::Deserialize;
/// use signalrs_derive::HubArgument;
/// # use signalrs_client::hub::Hub;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// #     let hub = Hub::default().method("Send", print);    
/// #     Ok(())
/// # }
/// # async fn print(_message: Data) {
/// #    // do nothing
/// # }
///
/// #[derive(Deserialize, HubArgument)]
/// struct Data {
///     f1: i32,
///     f2: String
/// }
/// ```
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
