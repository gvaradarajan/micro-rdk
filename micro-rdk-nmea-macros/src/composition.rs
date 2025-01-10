use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote, ToTokens};
use syn::{DeriveInput, Expr, Field, GenericArgument, Ident, Lit, PathArguments, Type};

use crate::attributes::MacroAttributes;
use crate::utils::{
    determine_supported_array, determine_supported_numeric, error_tokens,
    get_micro_nmea_crate_ident,
};

/// Represents a subset of auto-generated code statements for the implementation of a particular
/// NMEA message. Each field in a message struct contributes its own set of statements to the macro
/// sorted into buckets by category. Those statements are then merged into the set of statements
/// compiled by the previous field until the code for the message is complete, at which point the
/// composition can be turned into a TokenStream that can be returned by the macro function
pub(crate) struct PgnComposition {
    pub(crate) attribute_getters: Vec<TokenStream2>,
    pub(crate) parsing_logic: Vec<TokenStream2>,
    pub(crate) struct_initialization: Vec<TokenStream2>,
    pub(crate) proto_conversion_logic: Vec<TokenStream2>,
}

impl PgnComposition {
    pub(crate) fn new() -> Self {
        Self {
            attribute_getters: vec![],
            parsing_logic: vec![],
            struct_initialization: vec![],
            proto_conversion_logic: vec![],
        }
    }

    pub(crate) fn merge(&mut self, mut other: Self) {
        self.attribute_getters.append(&mut other.attribute_getters);
        self.parsing_logic.append(&mut other.parsing_logic);
        self.struct_initialization
            .append(&mut other.struct_initialization);
        self.proto_conversion_logic
            .append(&mut other.proto_conversion_logic);
    }

    pub(crate) fn from_field(field: &Field) -> Result<Self, TokenStream> {
        let mut statements = Self::new();
        if let Some(name) = &field.ident {
            if name == "source_id" {
                let num_ty = &field.ty;
                statements.attribute_getters.push(quote! {
                    pub fn #name(&self) -> #num_ty { self.#name }
                });
                statements
                    .struct_initialization
                    .push(quote! { source_id: source_id.unwrap(), });
                return Ok(statements);
            }

            let macro_attrs = MacroAttributes::from_field(field)?;
            if macro_attrs.offset != 0 {
                let offset = macro_attrs.offset;
                statements
                    .parsing_logic
                    .push(quote! { current_index += (#offset / 8) + 1; });
            }

            let new_statements = if field.attrs.iter().any(|attr| {
                attr.path()
                    .segments
                    .iter()
                    .any(|seg| seg.ident.to_string().as_str() == "fieldset")
            }) {
                handle_fieldset(name, field, &macro_attrs)?
            } else if determine_supported_numeric(&field.ty) {
                handle_number_field(name, field, &macro_attrs)?
            } else if macro_attrs.is_lookup {
                handle_lookup_field(name, &field.ty, &macro_attrs)?
            } else if determine_supported_array(&field.ty) {
                handle_array_field(name, &field.ty, &macro_attrs)?
            } else {
                let err_msg = format!(
                    "field type for {:?} unsupported for PGN message",
                    name.to_string()
                );
                return Err(error_tokens(&err_msg));
            };

            statements.merge(new_statements);
            Ok(statements)
        } else {
            Err(error_tokens(
                "could not parse parsing/getter statements for field",
            ))
        }
    }

    pub(crate) fn into_token_stream(self, input: &DeriveInput) -> TokenStream2 {
        let name = &input.ident;
        let parsing_logic = self.parsing_logic;
        let attribute_getters = self.attribute_getters;
        let struct_initialization = self.struct_initialization;
        let proto_conversion_logic = self.proto_conversion_logic;
        let (impl_generics, src_generics, src_where_clause) = input.generics.split_for_impl();
        let crate_ident = crate::utils::get_micro_nmea_crate_ident();
        let error_ident = quote! {#crate_ident::parse_helpers::errors::NmeaParseError};
        let mrdk_crate = crate::utils::get_micro_rdk_crate_ident();
        quote! {
            impl #impl_generics #name #src_generics #src_where_clause {
                pub fn from_bytes(data: &[u8], source_id: Option<u8>) -> Result<(Self, usize), #error_ident> {
                    use #crate_ident::parse_helpers::parsers::FieldReader;
                    #(#parsing_logic)*
                    Ok((Self {
                        #(#struct_initialization)*
                    }, current_index))
                }
                #(#attribute_getters)*

                pub fn to_readings(self) -> Result<#mrdk_crate::common::sensor::GenericReadingsResult, #error_ident> {
                    let mut readings = std::collections::HashMap::new();
                    #(#proto_conversion_logic)*
                    Ok(readings)
                }
            }
        }
    }

    pub(crate) fn into_fieldset_token_stream(self, input: &DeriveInput) -> TokenStream2 {
        let name = &input.ident;
        let parsing_logic = self.parsing_logic;
        let attribute_getters = self.attribute_getters;
        let struct_initialization = self.struct_initialization;
        let proto_conversion_logic = self.proto_conversion_logic;
        let (impl_generics, src_generics, src_where_clause) = input.generics.split_for_impl();
        let crate_ident = crate::utils::get_micro_nmea_crate_ident();
        let mrdk_crate = crate::utils::get_micro_rdk_crate_ident();
        let error_ident = quote! {#crate_ident::parse_helpers::errors::NmeaParseError};
        let field_set_ident = quote! {#crate_ident::parse_helpers::parsers::FieldSet};

        quote! {
            impl #impl_generics #name #src_generics #src_where_clause {
                #(#attribute_getters)*
            }

            impl #impl_generics #field_set_ident for #name #src_generics #src_where_clause {
                fn from_bytes(data: &[u8], current_index: usize) -> Result<(usize, Self), #error_ident> {
                    #(#parsing_logic)*
                    Ok((current_index, Self {
                        #(#struct_initialization)*
                    }))
                }

                fn to_readings(&self) -> Result<#mrdk_crate::common::sensor::GenericReadingsResult, #error_ident> {
                    let mut readings = std::collections::HashMap::new();
                    #(#proto_conversion_logic)*
                    Ok(readings)
                }
            }
        }
    }
}

fn handle_number_field(
    name: &Ident,
    field: &Field,
    macro_attrs: &MacroAttributes,
) -> Result<PgnComposition, TokenStream> {
    let bits_size: usize = macro_attrs.bits.unwrap();
    let scale_token = macro_attrs.scale_token.as_ref();
    let unit = macro_attrs.unit.as_ref();

    let num_ty = &field.ty;
    let mut new_statements = PgnComposition::new();
    let proto_import_prefix = crate::utils::get_proto_import_prefix();
    let prop_name = name.to_string();
    let label = macro_attrs.label.clone().unwrap_or(quote! {#prop_name});

    let crate_ident = crate::utils::get_micro_nmea_crate_ident();
    let error_ident = quote! {
        #crate_ident::parse_helpers::errors::NumberFieldError
    };
    let raw_fn_name = format_ident!("{}_raw", name);

    new_statements.attribute_getters.push(quote! {
        pub fn #raw_fn_name(&self) -> #num_ty { self.#name }
    });

    let mut return_type = quote! {#num_ty};
    let raw_value_statement = quote! {
        let mut result = self.#raw_fn_name();
    };
    let mut scaling_logic = quote! {};
    let mut unit_conversion_logic = quote! {};

    if let Some(scale_token) = scale_token {
        let name_as_string_ident = name.to_string();
        let max_token = match bits_size {
            8 | 16 | 32 | 64 => {
                quote! { <#num_ty>::MAX }
            }
            x => {
                let max_num = 2_i32.pow(x as u32);
                quote! { #max_num }
            }
        };
        scaling_logic = quote! {
            let result = match result {
                x if x == #max_token => { return Err(#error_ident::FieldNotPresent(#name_as_string_ident.to_string())); },
                x if x == (#max_token - 1) => { return Err(#error_ident::FieldError(#name_as_string_ident.to_string())); },
                x => {
                    (x as f64) * #scale_token
                }
            };
        };
        return_type = quote! {f64};
    }

    if let Some(unit) = unit {
        unit_conversion_logic = unit.tokens();
        return_type = quote! {f64};
    }

    new_statements.attribute_getters.push(quote! {
        pub fn #name(&self) -> Result<#return_type, #error_ident> {
            #raw_value_statement
            #scaling_logic
            #unit_conversion_logic
            Ok(result)
        }
    });

    new_statements.proto_conversion_logic.push(quote! {
        let value = #proto_import_prefix::Value {
            kind: Some(#proto_import_prefix::value::Kind::NumberValue(
                self.#name()? as f64
            ))
        };
        readings.insert(#label.to_string(), value);
    });

    let nmea_crate = get_micro_nmea_crate_ident();
    new_statements.parsing_logic.push(quote! {
        let reader = #nmea_crate::parse_helpers::parsers::NumberField::<#num_ty>::new(#bits_size)?;
        let (new_index, #name) = reader.read_from_data(&data[..], current_index)?;
        current_index = new_index;
    });

    new_statements.struct_initialization.push(quote! {#name,});
    Ok(new_statements)
}

fn handle_lookup_field(
    name: &Ident,
    field_type: &Type,
    macro_attrs: &MacroAttributes,
) -> Result<PgnComposition, TokenStream> {
    let mut new_statements = PgnComposition::new();
    let bits_size = macro_attrs.bits.unwrap();
    if let Type::Path(type_path) = field_type {
        let enum_type = type_path.clone();
        new_statements.attribute_getters.push(quote! {
            pub fn #name(&self) -> #enum_type { self.#name }
        });

        let nmea_crate = get_micro_nmea_crate_ident();
        let setters = quote! {
            let reader = #nmea_crate::parse_helpers::parsers::LookupField::<#enum_type>::new(#bits_size);
            let (new_index, #name) = reader.read_from_data(&data[..], current_index)?;
            current_index = new_index;
        };

        new_statements.parsing_logic.push(setters);

        new_statements.struct_initialization.push(quote! {#name,});
        let proto_import_prefix = crate::utils::get_proto_import_prefix();
        let prop_name = name.to_string();
        let label = macro_attrs.label.clone().unwrap_or(quote! {#prop_name});
        new_statements.proto_conversion_logic.push(quote! {
            let value = self.#name();
            let value = #proto_import_prefix::Value {
                kind: Some(#proto_import_prefix::value::Kind::StringValue(value.to_string()))
            };
            readings.insert(#label.to_string(), value);
        })
    }
    Ok(new_statements)
}

fn handle_array_field(
    name: &Ident,
    field_type: &Type,
    macro_attrs: &MacroAttributes,
) -> Result<PgnComposition, TokenStream> {
    let scale_token = macro_attrs.scale_token.as_ref();
    let byte_size = macro_attrs.bits.unwrap() / 8;
    if let Type::Array(type_array) = field_type {
        let num_ty = type_array.elem.to_token_stream();
        if let Expr::Lit(len_expr_lit) = &type_array.len {
            if let Lit::Int(len_lit) = &len_expr_lit.lit {
                let mut new_statements = PgnComposition::new();
                let len = match len_lit.base10_parse::<usize>() {
                    Ok(len) => len,
                    Err(_) => {
                        return Err(error_tokens("array type length parsing error"));
                    }
                };

                let nmea_crate = get_micro_nmea_crate_ident();
                new_statements.parsing_logic.push(quote! {
                    let reader = #nmea_crate::parse_helpers::parsers::ArrayField::<#num_ty, #len>::new();
                    let (new_index, #name) = reader.read_from_data(&data[..], current_index)?;
                    current_index = new_index;
                });

                new_statements.struct_initialization.push(quote! {#name,});

                if let Some(scale_token) = scale_token {
                    let raw_fn_name = format_ident!("{}_raw", name);
                    new_statements.attribute_getters.push(quote! {
                        pub fn #raw_fn_name(&self) -> [#num_ty; #len] {
                            self.#name
                        }
                    });
                    let is_unsigned = num_ty
                        .to_string()
                        .chars()
                        .next()
                        .unwrap_or_default()
                        .to_string()
                        == *"u";
                    new_statements.attribute_getters.push(match byte_size {
                        x if x <= 4 => {
                            let padding_len = 4 - x;
                            let over_type = if is_unsigned {
                                quote! {u32}
                            } else {
                                quote! {i32}
                            };
                            quote! {
                                pub fn #name(&self) -> Result<f64, std::array::TryFromSliceError> {
                                    let raw = self.#raw_fn_name();
                                    let padding: [u8; #padding_len] = [0; #padding_len];
                                    let full = [&raw[0..], &padding[0..]].concat();
                                    println!("full: {:?}", full);
                                    let full_slice: &[u8] = &full[0..];
                                    let num = <#over_type>::from_le_bytes(full_slice.try_into()?);
                                    Ok((num as f64) * #scale_token)
                                }
                            }
                        }
                        x if x <= 8 => {
                            let padding_len = 8 - x;
                            let over_type = if is_unsigned {
                                quote! {u64}
                            } else {
                                quote! {i64}
                            };
                            quote! {
                                pub fn #name(&self) -> Result<f64, std::array::TryFromSliceError> {
                                    let raw = self.#raw_fn_name();
                                    let padding: [u8; #padding_len] = [0; #padding_len];
                                    let full = [&raw[0..], &padding[0..]].concat();
                                    let full_slice: &[u8] = &full[0..];
                                    let num = <#over_type>::from_le_bytes(full_slice.try_into()?);
                                    Ok((num as f64) * #scale_token)
                                }
                            }
                        }
                        _ => {
                            return Err(error_tokens("something is wrong"));
                        }
                    });
                    let proto_import_prefix = crate::utils::get_proto_import_prefix();
                    let prop_name = name.to_string();
                    let label = macro_attrs.label.clone().unwrap_or(quote! {#prop_name});
                    new_statements.proto_conversion_logic.push(quote! {
                        let value = #proto_import_prefix::Value {
                            kind: Some(#proto_import_prefix::value::Kind::NumberValue(
                                self.#name()?
                            ))
                        };
                        readings.insert(#label.to_string(), value);
                    });
                }
                new_statements
                    .parsing_logic
                    .push(quote! { current_index += #byte_size; });
                Ok(new_statements)
            } else {
                Err(error_tokens(
                    "length of array property was unsupported type",
                ))
            }
        } else {
            Err(error_tokens(
                "length of array property was unsupported type",
            ))
        }
    } else {
        Err(error_tokens("bug found in determine_supported_array"))
    }
}

fn handle_fieldset(
    name: &Ident,
    field: &Field,
    macro_attrs: &MacroAttributes,
) -> Result<PgnComposition, TokenStream> {
    let mut new_statements = PgnComposition::new();
    if field.attrs.iter().any(|attr| {
        attr.path()
            .segments
            .iter()
            .any(|seg| seg.ident.to_string().as_str() == "fieldset")
    }) {
        let f_type = match &field.ty {
            Type::Path(type_path) => {
                let vec_seg = &type_path.path.segments[0];
                if &vec_seg.ident.to_string() != "Vec" {
                    Err(error_tokens("fieldset must be Vec"))
                } else {
                    if let PathArguments::AngleBracketed(args) = &vec_seg.arguments {
                        let type_arg = &args.args[0];
                        if let GenericArgument::Type(f_type) = type_arg {
                            Ok(f_type.to_token_stream())
                        } else {
                            Err(error_tokens("fieldset must be Vec with type"))
                        }
                    } else {
                        Err(error_tokens("fieldset must be Vec with angle brackets"))
                    }
                }
            }
            _ => Err(error_tokens("improper field type")),
        }?;

        let length_field_token = macro_attrs.length_field.as_ref().ok_or(error_tokens(
            "length_field field must be specified for fieldset",
        ))?;

        let nmea_crate = get_micro_nmea_crate_ident();
        new_statements.parsing_logic.push(quote! {
            let reader = #nmea_crate::parse_helpers::parsers::FieldSetList::<#f_type>::new(#length_field_token as usize);
            let (new_index, #name) = reader.read_from_data(&data[..], current_index)?;
            current_index = new_index;
        });

        new_statements.attribute_getters.push(quote! {
            pub fn #name(&self) -> Vec<#f_type> { self.#name.clone() }
        });
        new_statements.struct_initialization.push(quote! {#name,});
        let proto_import_prefix = crate::utils::get_proto_import_prefix();
        let prop_name = name.to_string();
        let label = macro_attrs.label.clone().unwrap_or(quote! {#prop_name});
        let crate_ident = crate::utils::get_micro_nmea_crate_ident();
        let error_ident = quote! {#crate_ident::parse_helpers::errors::NmeaParseError};
        new_statements.proto_conversion_logic.push(quote! {
            let values: Result<Vec<#proto_import_prefix::Value>, #error_ident> = self.#name().iter().map(|inst| {
                inst.to_readings().map(|fields| {
                    #proto_import_prefix::Value {
                        kind: Some(#proto_import_prefix::value::Kind::StructValue(#proto_import_prefix::Struct {
                            fields: fields
                        }))
                    }
                })
            }).collect();
            let value = #proto_import_prefix::Value {
                kind: Some(#proto_import_prefix::value::Kind::ListValue(#proto_import_prefix::ListValue {
                    values: values?
                }))
            };
            readings.insert(#label.to_string(), value);
        });
        Ok(new_statements)
    } else {
        Err(error_tokens("msg"))
    }
}
