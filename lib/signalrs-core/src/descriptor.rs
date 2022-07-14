use std::{collections::HashMap, sync::Arc};

use crate::{hub_response::HubResponse, hub_response::*};

#[allow(missing_debug_implementations)]
pub struct HubDescriptor<Hub, Ret> {
    pub hub: Arc<Hub>,
    pub methods: HashMap<String, MethodDescriptor<Hub, SignalRResponse<Ret>>>,
}

// impl<Hub, Ret> HubDescriptor<Hub, Ret>
// where
//     Ret: HubResponse,
// {
//     pub fn new(hub: Hub) -> Self {
//         HubDescriptor {
//             hub: Arc::new(hub),
//             methods: HashMap::new(),
//         }
//     }

//     pub fn method<Func, Args>(mut self, name: String, action: Func) -> Self
//     where
//         Func: Fn(&Hub, Args) -> Ret + 'static,
//         Ret: HubResponse + 'static,
//         Args: DeserializeOwned,
//     {
//         let method = move |hub: Arc<Hub>, input: String| -> Box<dyn HubResponse> {
//             let args = serde_json::from_str::<Args>(&input).unwrap(); // TODO: Fix me
//             Box::new(action(&hub, args))
//         };

//         self.methods.insert(name, MethodDescriptor::new(method));

//         self
//     }
// }

#[allow(missing_debug_implementations)]
pub struct MethodDescriptor<Hub, Ret> {
    pub action: Box<dyn Fn(Arc<Hub>, String) -> Ret>,
}

impl<Hub, Ret> MethodDescriptor<Hub, Ret>
where
    Ret: HubResponse,
{
    pub fn new<Func>(action: Func) -> Self
    where
        Func: Fn(Arc<Hub>, String) -> Ret + 'static,
    {
        MethodDescriptor {
            action: Box::new(action),
        }
    }
}
