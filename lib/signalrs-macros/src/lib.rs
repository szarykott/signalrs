use syn::{parse_macro_input, ItemImpl, ImplItemMethod};
use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn signalr_hub(args: TokenStream, input: TokenStream) -> TokenStream {
    let input_clone = input.clone();
    let parsed_input = parse_macro_input!(input_clone as ItemImpl);
    
    dbg!(parsed_input);

    input
}

#[proc_macro_attribute]
pub fn signalr_fn(args: TokenStream, input: TokenStream) -> TokenStream {
    let input_clone = input.clone();
    let parsed_input = parse_macro_input!(input_clone as ImplItemMethod);
    
    // dbg!(parsed_input);

    input
}
