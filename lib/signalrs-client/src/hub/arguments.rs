use super::invocation::{ArgumentsLeft, ExtractionError, FromInvocation, HubInvocation};

macro_rules! impl_from_invocation {
    ($($ty:ident),+) => {
        $(
            impl FromInvocation for $ty {
                fn try_from_invocation(request: &mut HubInvocation) -> Result<Self, ExtractionError> {
                    match &mut request.state.arguments {
                        ArgumentsLeft::Text(args) => {
                            let next = args.next().ok_or_else(|| ExtractionError::MissingArgs)?;
                            serde_json::from_value(next).map_err(|e| e.into())
                        },
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
