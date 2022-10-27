pub mod builder;
mod functions;

use std::{collections::HashMap, pin::Pin, sync::Arc};

use self::functions::Callable;
use futures::Future;

use super::error::SignalRError;

pub struct Hub {
    pub(crate) methods: HashMap<
        String,
        Arc<
            dyn Callable<Future = Pin<Box<dyn Future<Output = Result<(), SignalRError>> + Send>>>
                + Send
                + Sync,
        >,
    >,
}
