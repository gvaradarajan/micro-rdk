use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::{format_ident, quote};
use syn::{Field, Ident, Meta, Path, Type, TypePath};

fn get_micro_nmea_crate_ident() -> Ident {
    let found_crate =
        crate_name("micro-rdk-nmea").expect("micro-rdk-nmea is present in `Cargo.toml`");
    match found_crate {
        FoundCrate::Itself => Ident::new("crate", Span::call_site()),
        FoundCrate::Name(name) => Ident::new(&name, Span::call_site()),
    }
}

fn determine_supported_numeric(field_type: &Type) -> bool {
    match field_type {
        Type::Path(type_path) => {
            type_path.path.is_ident("u32")
                || type_path.path.is_ident("u16")
                || type_path.path.is_ident("u8")
                || type_path.path.is_ident("i32")
                || type_path.path.is_ident("i16")
                || type_path.path.is_ident("i8")
        }
        _ => false,
    }
}

fn determine_supported_enum(field_type: &Type) -> Option<TokenStream2> {
    match field_type {
        Type::Path(type_path) if type_path.path.is_ident("WaterReference") => {
            let ty = Type::Path(TypePath {
                qself: None,
                path: Path::from(Ident::new("WaterReference", Span::call_site())),
            });
            Some(quote!(#ty))
        }
        _ => None,
    }
}

fn get_field_size(field_type: &Type) -> usize {
    match field_type {
        Type::Path(type_path) if type_path.path.is_ident("u32") => 4,
        Type::Path(type_path) if type_path.path.is_ident("u16") => 2,
        Type::Path(type_path) if type_path.path.is_ident("u8") => 1,
        Type::Path(type_path) if type_path.path.is_ident("i32") => 4,
        Type::Path(type_path) if type_path.path.is_ident("i16") => 2,
        Type::Path(type_path) if type_path.path.is_ident("i8") => 1,
        _ => 0,
    }
}

fn error_tokens(msg: &str) -> TokenStream {
    syn::Error::new(Span::call_site(), msg)
        .to_compile_error()
        .into()
}

fn handle_number_field(
    name: &Ident,
    field: &Field,
    current_idx: usize,
    byte_size: usize,
) -> Result<(Vec<TokenStream2>, Vec<TokenStream2>, Vec<TokenStream2>), TokenStream> {
    let mut scale_found = false;
    let num_ty = &field.ty;
    let mut attr_statements = vec![];
    let mut setter_statements = vec![];
    let mut struct_attrs = vec![];
    for attr in field.attrs.iter() {
        if attr
            .path
            .segments
            .iter()
            .find(|seg| {
                let ident = &seg.ident;
                ident.to_string() == "scale".to_string()
            })
            .is_some()
        {
            scale_found = true;
            let meta = attr.parse_meta();

            let scale_token = match &meta {
                Ok(Meta::NameValue(named)) => {
                    if let syn::Lit::Float(ref scale_lit) = named.lit {
                        quote!(#scale_lit)
                    } else {
                        return Err(error_tokens("scale parameter must be float"));
                    }
                }
                _ => {
                    return Err(error_tokens("scale received unexpected attribute value"));
                }
            };
            let raw_fn_name = format_ident!("{}_raw", name);
            let crate_ident = get_micro_nmea_crate_ident();
            let error_ident = quote! {
                #crate_ident::parse_helpers::errors::NumberFieldError
            };
            let name_as_string_ident = name.to_string();
            attr_statements.push(quote! {
                pub fn #name(&self) -> Result<f64, #error_ident> {
                    match self.#name {
                        x if x == <#num_ty>::MAX => Err(#error_ident::FieldNotPresent(#name_as_string_ident.to_string())),
                        x if x == (<#num_ty>::MAX - 1) => Err(#error_ident::FieldError(#name_as_string_ident.to_string())),
                        _ => {
                            Ok((self.#name as f64) * #scale_token)
                        }
                    }
                }
            });
            attr_statements.push(quote! {
                pub fn #raw_fn_name(&self) -> #num_ty { self.#name }
            });
        }
    }
    if !scale_found {
        attr_statements.push(quote! {
            fn #name(&self) -> #num_ty { self.#name }
        });
    }
    let end_idx = current_idx + byte_size;
    setter_statements.push(quote! {
        let #name: &[u8] = &data[#current_idx..#end_idx];
        let #name = <#num_ty>::from_le_bytes(#name.try_into()?);
    });
    struct_attrs.push(quote! {#name,});
    Ok((attr_statements, setter_statements, struct_attrs))
}

/// PgnMessageDerive is a macro that implements parsing logic in the form of a method
/// `from_bytes(Vec<u8>) -> Result<Self>` and attribute accessors for a struct representing
/// an NMEA 2K PGN message. It requires that the struct define the fields in order of
/// appearance in the bytes representation. Reference or lookup fields should have the
/// corresponding enum defined in micro-rdk-nmea/src/parse_helpers/enum.rs as its data type.
/// Here's an example of a defined PGN and the code auto-generated by the macro
///
/// PGN message
///
/// #[derive(PgnMessageDerive)]
/// pub struct Speed {
///     source_id: u8,
///     #[scale = 0.01] speed_water_ref: u16,
///     #[scale = 0.01] speed_ground_ref: u16,
///     speed_water_ref_type: WaterReference
/// }
///
/// Generated code
///
/// pub struct Speed {
///     source_id: u8,
///     #[scale = 0.01]
///     speed_water_ref: u16,
///     #[scale = 0.01]
///     speed_ground_ref: u16,
///     speed_water_ref_type: WaterReference,
/// }
/// impl Speed {
///     pub fn from_bytes(
///         data: Vec<u8>,
///         source_id: u8,
///     ) -> Result<Self, std::array::TryFromSliceError> {
///         let speed_water_ref: &[u8] = &data[0usize..2usize];
///         let speed_water_ref = <u16>::from_le_bytes(speed_water_ref.try_into()?);
///         let speed_ground_ref: &[u8] = &data[2usize..4usize];
///         let speed_ground_ref = <u16>::from_le_bytes(
///             speed_ground_ref.try_into()?,
///         );
///         let speed_water_ref_type = <WaterReference>::from_byte(data[4usize]);
///         Ok(Self {
///             source_id,
///             speed_water_ref,
///             speed_ground_ref,
///             speed_water_ref_type,
///         })
///     }
///     pub fn source_id(&self) -> u8 {
///         self.source_id
///     }
///     pub fn speed_water_ref(
///         &self,
///     ) -> Result<f64, crate::parse_helpers::errors::NumberFieldError> {
///         match self.speed_water_ref {
///             x if x == <u16>::MAX => {
///                 Err(
///                     crate::parse_helpers::errors::NumberFieldError::FieldNotPresent(
///                         "speed_water_ref".to_string(),
///                     ),
///                 )
///             }
///             x if x == (<u16>::MAX - 1) => {
///                 Err(
///                     crate::parse_helpers::errors::NumberFieldError::FieldError(
///                         "speed_water_ref".to_string(),
///                     ),
///                 )
///             }
///             _ => Ok((self.speed_water_ref as f64) * 0.01),
///         }
///     }
///     pub fn speed_water_ref_raw(&self) -> u16 {
///         self.speed_water_ref
///     }
///     pub fn speed_ground_ref(
///         &self,
///     ) -> Result<f64, crate::parse_helpers::errors::NumberFieldError> {
///         match self.speed_ground_ref {
///             x if x == <u16>::MAX => {
///                 Err(
///                     crate::parse_helpers::errors::NumberFieldError::FieldNotPresent(
///                         "speed_ground_ref".to_string(),
///                     ),
///                 )
///             }
///             x if x == (<u16>::MAX - 1) => {
///                 Err(
///                     crate::parse_helpers::errors::NumberFieldError::FieldError(
///                         "speed_ground_ref".to_string(),
///                     ),
///                 )
///             }
///             _ => Ok((self.speed_ground_ref as f64) * 0.01),
///         }
///     }
///     pub fn speed_ground_ref_raw(&self) -> u16 {
///         self.speed_ground_ref
///     }
///     pub fn speed_water_ref_type(&self) -> WaterReference {
///         self.speed_water_ref_type
///     }
/// }
#[proc_macro_derive(PgnMessageDerive, attributes(scale))]
pub fn pgn_message_derive(item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::DeriveInput);
    let name = input.ident;

    let src_fields = if let syn::Data::Struct(syn::DataStruct { fields, .. }) = input.data {
        fields
    } else {
        return syn::Error::new(Span::call_site(), "PgnMessageDerive expected struct")
            .to_compile_error()
            .into();
    };

    let named_fields = if let syn::Fields::Named(f) = src_fields {
        f.named
    } else {
        return error_tokens("PgnMessageDerive expected struct with named fields");
    };

    let mut attr_statements = vec![];
    let mut setter_statements = vec![];
    let mut struct_attrs = vec![];
    let mut current_index = 0;
    for field in named_fields.iter() {
        if let Some(name) = &field.ident {
            if name.to_string() == "source_id".to_string() {
                let num_ty = &field.ty;
                attr_statements.push(quote! {
                    pub fn #name(&self) -> #num_ty { self.#name }
                });
                continue;
            }
            if determine_supported_numeric(&field.ty) {
                let byte_size = get_field_size(&field.ty);
                let (
                    mut field_attr_statements,
                    mut field_setter_statements,
                    mut field_struct_attrs,
                ) = match handle_number_field(name, field, current_index, byte_size) {
                    Ok(res) => res,
                    Err(err) => {
                        return err;
                    }
                };
                attr_statements.append(&mut field_attr_statements);
                setter_statements.append(&mut field_setter_statements);
                struct_attrs.append(&mut field_struct_attrs);
                current_index += byte_size;
            } else if let Some(enum_type) = determine_supported_enum(&field.ty) {
                attr_statements.push(quote! {
                    pub fn #name(&self) -> #enum_type { self.#name }
                });
                setter_statements.push(quote! {
                    let #name = <#enum_type>::from_byte(data[#current_index]);
                });
                struct_attrs.push(quote! {#name,});
                current_index += 1;
            }
        };
    }

    let (impl_generics, src_generics, src_where_clause) = input.generics.split_for_impl();
    let gen = quote! {
        impl #impl_generics #name #src_generics #src_where_clause {
            pub fn from_bytes(data: Vec<u8>, source_id: u8) -> Result<Self, std::array::TryFromSliceError> {
                #(#setter_statements)*
                Ok(Self {
                    source_id,
                    #(#struct_attrs)*
                })
            }
            #(#attr_statements)*
        }
    };
    gen.into()
}
