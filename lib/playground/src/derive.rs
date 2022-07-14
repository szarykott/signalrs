use signalrs_core::hub_response::*;
use signalrs_macros::{describe, signalr_hub};

pub struct Test;

// #[signalr_hub]
#[describe]
impl Test {
    pub fn number(self) -> impl HubResponse {
        1
    }
}
