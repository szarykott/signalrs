use proc_macro::{self, TokenStream};
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(FromInvocation)]
pub fn derive(input: TokenStream) -> TokenStream {
    let DeriveInput { ident, .. } = parse_macro_input!(input);
    let output = quote! {
        impl signalrs_client::hub::FromInvocation for #ident {
            fn try_from_invocation(request: &mut signalrs_client::hub::HubInvocation) -> Result<Self, signalrs_client::hub::ExtractionError> {
                match &mut request.get_arguments() {
                    signalrs_client::hub::ArgumentsLeft::Text(args) => {
                        let next = args.next().ok_or_else(|| signalrs_client::hub::ExtractionError::MissingArgs)?;
                        serde_json::from_value(next).map_err(|e| e.into())
                    },
                }
            }
        }
    };
    output.into()
}
