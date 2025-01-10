pub(crate) mod attributes;
pub(crate) mod composition;
pub(crate) mod utils;

use crate::composition::PgnComposition;
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::DeriveInput;

fn get_statements(
    input: &DeriveInput,
    initialize_current_index: bool,
) -> Result<PgnComposition, TokenStream> {
    let src_fields = if let syn::Data::Struct(syn::DataStruct { ref fields, .. }) = input.data {
        fields
    } else {
        return Err(
            syn::Error::new(Span::call_site(), "PgnMessageDerive expected struct")
                .to_compile_error()
                .into(),
        );
    };

    let named_fields = if let syn::Fields::Named(f) = src_fields {
        &f.named
    } else {
        return Err(crate::utils::error_tokens(
            "PgnMessageDerive expected struct with named fields",
        ));
    };

    let mut statements = PgnComposition::new();
    if initialize_current_index {
        statements
            .parsing_logic
            .push(quote! { let mut current_index: usize = 0; });
    } else {
        statements
            .parsing_logic
            .push(quote! { let mut current_index: usize = current_index; });
    }
    for field in named_fields.iter() {
        match PgnComposition::from_field(field) {
            Ok(new_statements) => {
                statements.merge(new_statements);
            }
            Err(err) => {
                return Err(err);
            }
        };
    }

    Ok(statements)
}

/// PgnMessageDerive is a macro that implements parsing logic in the form of a method
/// `from_bytes(Vec<u8>) -> Result<Self>` and attribute accessors for a struct representing
/// an NMEA 2K PGN message.
#[proc_macro_derive(
    PgnMessageDerive,
    attributes(label, scale, lookup, bits, offset, fieldset, length_field, unit)
)]
pub fn pgn_message_derive(item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::DeriveInput);

    match get_statements(&input, true) {
        Ok(gen) => gen.into_token_stream(&input).into(),
        Err(tokens) => tokens,
    }
}
