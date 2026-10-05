//! Derives for fmite. Each writes the same impl you would write by hand, using fmite's
//! public API, so it adds no rules of its own. `Variable::new::<FieldType>` checks
//! Table 22 for derived and hand-written lists alike.

#![deny(clippy::all)]
#![warn(clippy::pedantic)]

use std::fmt::Write as _;

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as Tokens};
use quote::{ToTokens, quote};
use syn::{Data, DeriveInput, Error, Expr, Fields, Lit, Type, UnOp, parse_macro_input};

/// The possible field types of a variable.
const VARIABLE_TYPES: [&str; 6] = [
    "Input",
    "Output",
    "Parameter",
    "CalculatedParameter",
    "Local",
    "Field",
];

/// Implements `fmite::Variables` for a struct. Each field of type `Input`, `Output`,
/// `Parameter`, `CalculatedParameter` or `Local` is a variable, numbered in declaration
/// order from 1 (`time` is 0). `MODEL_NAME` is the crate's package name, and
/// `INSTANTIATION_TOKEN` is a hash of the variable declarations.
#[proc_macro_derive(Variables)]
pub fn derive_variables(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    variables(&input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

fn is_variable(ty: &Type) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };
    (path.path.segments.last())
        .is_some_and(|segment| VARIABLE_TYPES.iter().any(|name| segment.ident == name))
}

fn variables(input: &DeriveInput) -> syn::Result<Tokens> {
    let Data::Struct(data) = &input.data else {
        return Err(Error::new_spanned(
            input,
            "`Variables` derives for a struct",
        ));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(Error::new_spanned(
            input,
            "`Variables` derives for a struct with named fields",
        ));
    };
    let mut declarations = Vec::new();
    let mut gets = Vec::new();
    let mut sets = Vec::new();
    let mut signature = input.ident.to_string();
    let variables = (fields.named.iter()).filter(|field| is_variable(&field.ty));
    for (field, vr) in variables.zip(1_u32..) {
        let ident = field.ident.as_ref().expect("named fields");
        let ty = &field.ty;
        let name = ident.to_string();
        let _ = write!(signature, ";{name}:{}", ty.to_token_stream());
        declarations.push(quote! { ::fmite::Variable::new::<#ty>(#name, #vr) });
        gets.push(quote! { #vr => self.#ident.get_into(out) });
        sets.push(quote! { #vr => self.#ident.importer_set(values) });
    }
    let token = token(&signature);
    let ident = &input.ident;
    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics ::fmite::Variables for #ident #type_generics #where_clause {
            const MODEL_NAME: &'static str = ::core::env!("CARGO_PKG_NAME");
            const INSTANTIATION_TOKEN: &'static str = #token;
            const VARIABLES: &'static [::fmite::Variable] = &[#(#declarations),*];

            fn get(
                &self,
                vr: ::fmite::ValueReference,
                out: ::fmite::ValuesMut<'_>,
            ) -> ::core::result::Result<(), ::fmite::Error> {
                match vr.0 {
                    #(#gets,)*
                    _ => ::core::result::Result::Err(vr.unknown()),
                }
            }

            fn set(
                &mut self,
                vr: ::fmite::ValueReference,
                values: ::fmite::Values<'_>,
            ) -> ::core::result::Result<(), ::fmite::Error> {
                match vr.0 {
                    #(#sets,)*
                    _ => ::core::result::Result::Err(vr.unknown()),
                }
            }
        }
    })
}

/// A GUID-shaped token: 128 bits of FNV-1a over the declarations, from two offsets.
fn token(signature: &str) -> String {
    let fnv = |offset: u64| {
        signature.bytes().fold(offset, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
    };
    let hex = format!(
        "{:016x}{:016x}",
        fnv(0xcbf2_9ce4_8422_2325),
        fnv(0x8422_2325_cbf2_9ce4)
    );
    format!(
        "{{{}-{}-{}-{}-{}}}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// Implements `fmite::Enumeration` for an enum of unit variants. The items are the
/// variants, valued by their discriminants. An explicit discriminant must be an integer
/// literal. As in Rust, a variant without one is the previous value plus one, and the
/// first is 0.
#[proc_macro_derive(Enumeration)]
pub fn derive_enumeration(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    enumeration(&input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

fn discriminant(expr: &Expr) -> syn::Result<i64> {
    let literal = |expr: &Expr| match expr {
        Expr::Lit(lit) => match &lit.lit {
            Lit::Int(int) => int.base10_parse::<i64>(),
            other => Err(Error::new_spanned(other, "an integer discriminant")),
        },
        other => Err(Error::new_spanned(other, "an integer literal discriminant")),
    };
    match expr {
        Expr::Unary(unary) if matches!(unary.op, UnOp::Neg(_)) => Ok(-literal(&unary.expr)?),
        expr => literal(expr),
    }
}

fn enumeration(input: &DeriveInput) -> syn::Result<Tokens> {
    let Data::Enum(data) = &input.data else {
        return Err(Error::new_spanned(
            input,
            "`Enumeration` derives for an enum",
        ));
    };
    let mut items = Vec::new();
    let mut next = 0_i64;
    for variant in &data.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(Error::new_spanned(
                variant,
                "an enumeration item has no fields",
            ));
        }
        let value = match &variant.discriminant {
            Some((_, expr)) => discriminant(expr)?,
            None => next,
        };
        next = value
            .checked_add(1)
            .ok_or_else(|| Error::new(Span::call_site(), "a discriminant overflows i64"))?;
        items.push((&variant.ident, value));
    }
    let ident = &input.ident;
    let name = ident.to_string();
    let names = items.iter().map(|(variant, _)| variant.to_string());
    let values: Vec<_> = items.iter().map(|(_, value)| *value).collect();
    let variants: Vec<_> = items.iter().map(|(variant, _)| *variant).collect();
    Ok(quote! {
        impl ::fmite::Enumeration for #ident {
            const NAME: &'static str = #name;
            const ITEMS: &'static [(&'static str, i64)] = &[#((#names, #values)),*];

            fn to_i64(self) -> i64 {
                match self {
                    #(Self::#variants => #values,)*
                }
            }

            fn from_i64(value: i64) -> ::core::option::Option<Self> {
                match value {
                    #(#values => ::core::option::Option::Some(Self::#variants),)*
                    _ => ::core::option::Option::None,
                }
            }
        }
    })
}
