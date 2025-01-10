use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use syn::{Ident, Type};

pub(crate) enum UnitConversion {
    KelvinToCelsius,
    CoulombToAmpereHour,
    PascalToBar,
    RadianToDegree,
    RadPerSecToDegPerSec,
}

impl TryFrom<String> for UnitConversion {
    type Error = TokenStream;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "Ah" => Ok(Self::CoulombToAmpereHour),
            "bar" => Ok(Self::PascalToBar),
            "C" => Ok(Self::KelvinToCelsius),
            "deg" => Ok(Self::RadianToDegree),
            "deg/s" => Ok(Self::RadPerSecToDegPerSec),
            x => Err(error_tokens(
                format!("encountered unsupported unit {:?}", x).as_str(),
            )),
        }
    }
}

impl UnitConversion {
    pub(crate) fn tokens(&self) -> TokenStream2 {
        match self {
            Self::KelvinToCelsius => quote! {
                let result = (result as f64) - 273.15;
            },
            Self::CoulombToAmpereHour => quote! {
                let result = (result as f64) / 3600;
            },
            Self::PascalToBar => quote! {
                let result = (result as f64) / 100000.0;
            },
            Self::RadianToDegree | Self::RadPerSecToDegPerSec => quote! {
                let result = (result as f64) * (180.0 / std::f64::consts::PI);
            },
        }
    }
}

pub(crate) fn error_tokens(msg: &str) -> TokenStream {
    syn::Error::new(Span::call_site(), msg)
        .to_compile_error()
        .into()
}

pub(crate) fn get_micro_nmea_crate_ident() -> Ident {
    let found_crate =
        crate_name("micro-rdk-nmea").expect("micro-rdk-nmea is present in `Cargo.toml`");
    match found_crate {
        FoundCrate::Itself => Ident::new("crate", Span::call_site()),
        FoundCrate::Name(name) => Ident::new(&name, Span::call_site()),
    }
}

pub(crate) fn get_micro_rdk_crate_ident() -> Ident {
    let found_crate = crate_name("micro-rdk").expect("micro-rdk is present in `Cargo.toml`");
    match found_crate {
        FoundCrate::Itself => Ident::new("crate", Span::call_site()),
        FoundCrate::Name(name) => Ident::new(&name, Span::call_site()),
    }
}

pub(crate) fn get_proto_import_prefix() -> TokenStream2 {
    let crate_ident = get_micro_rdk_crate_ident();
    quote! {#crate_ident::google::protobuf}
}

pub(crate) fn determine_supported_numeric(field_type: &Type) -> bool {
    match field_type {
        Type::Path(type_path) => {
            type_path.path.is_ident("u32")
                || type_path.path.is_ident("u16")
                || type_path.path.is_ident("u8")
                || type_path.path.is_ident("i32")
                || type_path.path.is_ident("i16")
                || type_path.path.is_ident("i64")
                || type_path.path.is_ident("u64")
                || type_path.path.is_ident("i8")
        }
        _ => false,
    }
}

pub(crate) fn determine_supported_array(field_type: &Type) -> bool {
    match field_type {
        Type::Array(type_array) => match type_array.elem.as_ref() {
            Type::Path(inner_type_path) => {
                inner_type_path.path.is_ident("u32")
                    || inner_type_path.path.is_ident("u16")
                    || inner_type_path.path.is_ident("u8")
                    || inner_type_path.path.is_ident("i32")
                    || inner_type_path.path.is_ident("i16")
                    || inner_type_path.path.is_ident("i8")
            }
            _ => false,
        },
        _ => false,
    }
}
