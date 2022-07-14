// let hub = HubDescriptor::<State>::new()
//     .method::<Args>(|s, args|  s.method1(args))
//     .method::<Args2>(|s, args2|, s.method2(args2))
//     .build()

// let hub = HubDescriptor::<State>::new()
//     .method::<Args>(|s| s.method1.describe())
//     .method::<Args2>(|s, args2|, s.method2(args2))
//     .build()

pub trait DescribeMethod {
    fn describe(&self) -> MethodDescriptor;
}

/*
let hub = HubDescriptor::new()
    .state(<sth>)
    .state(<sth>)
    .method("name", <method1>)
    .method("name2", <method2>)
    .streaming_method("n3", <method4>)

fn method1(Args(a,b) : Args<i32, u32>) {

}

fn method2(Args(a) : Args<u32>, state : State<u32>) {

}

fn method21(Args(a) : Args<u32>, state : State<Clients>) {

}

czy chcę robić statemut?
fn method2(Args(a) : Args<u32>, state : StateMut<u32>) {

}

fn

hub.run().await ???
*/
