use proc_macro::TokenStream;
use proc_macro2;
use quote::quote;
use syn::{parse_macro_input, ItemImpl};

#[proc_macro_attribute]
pub fn signalr_hub(args: TokenStream, input: TokenStream) -> TokenStream {
    let input_clone = input.clone();

    dbg!(input_clone);

    input
}

#[proc_macro_attribute]
pub fn signalr_fn(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = proc_macro2::TokenStream::from(input);
    TokenStream::from(quote! { #input fn dupa() {} })
}
