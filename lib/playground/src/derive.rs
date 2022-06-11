use signalrs_core::hub_response::*;
use signalrs_macros::{signalr_fn, signalr_hub};

pub struct Test;

#[signalr_hub]
impl Test {
    #[signalr_fn]
    pub fn number() -> impl HubResponse {
        1
    }
}
