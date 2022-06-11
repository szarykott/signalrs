use signalrs_macros::{signalr_hub, signalr_fn};
use signalrs_core::hub_response::*;

pub struct Test;

#[signalr_hub]
impl Test {

    #[signalr_fn]
    pub fn number() -> impl HubResponse  {
        1
    }
}

