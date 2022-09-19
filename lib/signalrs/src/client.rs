use crate::protocol::Invocation;
use futures::sink::{Sink, SinkExt};
use serde::Serialize;
use std::fmt::Debug;
use uuid::Uuid;

pub struct SignalRClient<S> {
    sink: S,
}

impl<S> SignalRClient<S>
where
    S: Sink<String> + Unpin,
    <S as Sink<String>>::Error: Debug,
{
    pub fn invoke(method: String) {
        let _invocation = Invocation::<()>::without_id(method, None);
    }

    pub fn invoke1<T>(method: String, arg1: T) {
        let _invocation = Invocation::without_id(method, Some((arg1,)));
    }

    pub async fn invoke2<T1, T2>(&mut self, method: String, arg1: T1, arg2: T2)
    where
        T1: Serialize,
        T2: Serialize,
    {
        let invocation =
            Invocation::with_id(Uuid::new_v4().to_string(), method, Some((arg1, arg2)));
        let invocation_json = serde_json::to_string(&invocation).unwrap();
        self.sink.send(invocation_json).await.unwrap();
    }
}
