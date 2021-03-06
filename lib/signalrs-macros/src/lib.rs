use proc_macro::TokenStream as CompilerTokenStream;
use proc_macro2::TokenStream as MacroTokenStream;
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, ImplItem, ImplItemMethod, ItemImpl, ReturnType, Type, TypeParamBound,
};

// it hurts that there is not typechecking ...
const HUB_RESPONSE_TYPE: &'static str = stringify!(HubResponse);
const HUB_STREAM_TYPE: &'static str = stringify!(HubStream);

#[proc_macro_attribute]
pub fn describe(_args: CompilerTokenStream, input: CompilerTokenStream) -> CompilerTokenStream {
    let input_clone = input.clone();

    let impl_block = parse_macro_input!(input as ItemImpl);

    dbg!(impl_block);

    input_clone
}

#[proc_macro_attribute]
pub fn signalr_hub(_args: CompilerTokenStream, input: CompilerTokenStream) -> CompilerTokenStream {
    let original_input = proc_macro2::TokenStream::from(input.clone());

    let impl_block = parse_macro_input!(input as ItemImpl);

    let struct_self_type = *impl_block.self_ty;
    let methods = generate_method_descriptors(&struct_self_type, impl_block.items);

    CompilerTokenStream::from(quote! {
        #original_input

        impl #struct_self_type {
            #methods
        }
    })
}

fn generate_method_descriptors(self_type: &Type, items: Vec<ImplItem>) -> MacroTokenStream {
    let mut output = MacroTokenStream::new();

    for item in items {
        match item {
            ImplItem::Method(method) => {
                if is_hub_method(&method) {
                    output.extend(generate_method_descriptor(self_type, method));
                }
            }
            _ => {}
        }
    }

    output
}

fn is_hub_method(method: &ImplItemMethod) -> bool {
    is_public(method) && is_hub_response_type(method)
}

fn is_public(method: &ImplItemMethod) -> bool {
    match method.vis {
        syn::Visibility::Public(_) => true,
        _ => false,
    }
}

fn is_hub_response_type(method: &ImplItemMethod) -> bool {
    if let ReturnType::Type(_, ret_type) = &method.sig.output {
        if let Type::ImplTrait(impl_trait) = &**ret_type {
            let bounds = &impl_trait.bounds;

            if bounds.len() != 1 {
                return false;
            }

            if let TypeParamBound::Trait(bound) = bounds.first().unwrap() {
                let segments = &bound.path.segments;

                if segments.len() != 1 {
                    return false;
                }

                let segment = segments.first().unwrap();

                let ident = &segment.ident;

                return ident == HUB_RESPONSE_TYPE || ident == HUB_STREAM_TYPE;
            }
        }
    }

    false
}

fn generate_method_descriptor(self_type: &Type, method: ImplItemMethod) -> MacroTokenStream {
    let method_name = format_ident!("{}_descriptor", method.sig.ident);
    let args = method.sig.inputs;

    quote! {
        pub fn #method_name<Out>(#args) -> signalrs_core::descriptor::MethodDescriptor<#self_type, Out>
        where
            Out: futures::sink::Sink<String> + std::marker::Send + 'static + Unpin + Clone,
            <Out as futures::sink::Sink<String>>::Error: std::fmt::Debug + std::error::Error,
        {
            MethodDescriptor::new(
                Box::new(|hub, text, output| {
                    Box::pin(text_invocation(text, move |arg| hub.do_it(arg), output))
                })
            )
        }
    }
}

// pub fn do_it_descriptor<Out>() -> MethodDescriptor<DaHub, Out>
// where
//     Out: Sink<String> + Send + 'static + Unpin + Clone,
//     <Out as Sink<String>>::Error: Debug + std::error::Error,
// {
//     // MethodDescriptor {
//     //     action: Box::new(|hub, text, output| {
//     //         Box::pin(text_invocation(text, move |arg| hub.do_it(arg), output))
//     //     }),
//     // }

//     todo!()
// }
