use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemStruct, parse_macro_input};

#[proc_macro_attribute]
pub fn blenny_module(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemStruct);
    let name = &input.ident;

    let expanded = quote! {
        #input

        // Submit this module into the global inventory.
        inventory::submit! {
            blenny::ModuleRegistration {
                name: stringify!(#name),
                constructor: || Box::new(#name::default()),
            }
        }
    };

    TokenStream::from(expanded)
}
