//! This crate exposes the 'Candle' macro,
//! It combines multiple types that implement 'CandleComponent'
//! into a single 'ModularCandle' struct
//! which can then be used as the output type of some aggregation process.
//! It also exposes getter methods for each 'CandleComponent' for convenience.
//! The name of the getter method is equivalent to the field name.
//! e.g.:
//! struct MyCandle {
//!    open: Open,
//! }
//! with the derive macro will create a "fn open(&self)" method which gets the inner value
//!
//! When deriving the 'Candle' macro, make sure the following things are in scope:
//! - Trade
//! - ModularCandle
//! - CandleComponent

#![deny(missing_docs)]

use std::path::Path;

use proc_macro::TokenStream;
use quote::{__private::Span, quote};
use syn::{
    self, AngleBracketedGenericArguments, Data, DataStruct, Fields, GenericArgument, Ident,
    PathSegment, Type, TypePath,
};

/// The 'Candle' macro takes a named struct,
/// that has multiple fields of type 'CandleComponent'
/// to automatically generate a struct that implements
/// the 'ModularCandle' trait, which means it can then be used
/// in the aggregation process.
/// It also exposes getter functions for each 'CandleComponent' for convenience.
#[proc_macro_derive(Candle)]
pub fn candle_macro_derive(input: TokenStream) -> TokenStream {
    // Construct a representation of Rust code as a syntax tree
    // that we can manipulate
    let ast = syn::parse(input).unwrap();

    // Build the trait implementation
    impl_candle_macro(&ast)
}

fn phantom_path_to_type(path: &syn::Path) -> Option<Ident> {
    if path.leading_colon.is_some() {
        return None;
    }
    let mut it = path.segments.iter();
    let segment = it.next()?;
    match &segment.arguments {
        syn::PathArguments::AngleBracketed(AngleBracketedGenericArguments { args: x, .. }) => {
            match x.first() {
                Some(GenericArgument::Type(Type::Path(TypePath {
                    path: syn::Path { segments: segs, .. },
                    ..
                }))) => segs.first().and_then(|x| Some(x.ident.clone())),
                _ => None,
            }
        }
        _ => None,
    }
}

fn impl_candle_macro(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let components = match &ast.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => &fields.named,
        _ => panic!("Use a named struct"),
    };

    let input_field = components
        .iter()
        .map(|x| x)
        .filter(|x| format!("{}", x.ident.clone().unwrap()) == "input")
        .next();
    let input_type = match input_field {
        Some(syn::Field {
            ty: syn::Type::Path(syn::TypePath { path: p, .. }),
            ..
        }) => phantom_path_to_type(&p),
        None => Some(Ident::new("Trade", Span::call_site())),
        _ => panic!("input attribute is expected to be PhantomData<Input>"),
    };

    let fn_names0 = components
        .iter()
        .filter(|v| format!("{}", v.ident.clone().unwrap()) != "input")
        .map(|v| v.ident.clone().unwrap());
    let fn_names1 = fn_names0.clone();
    let fn_names2 = fn_names1.clone();
    let input_name = input_type.expect("No phantom data for input type!");

    let gen = quote! {
        impl #name {
            #(
                pub fn #fn_names0(&self) -> f64 {
                    self.#fn_names0.value()
                }
            )*
        }

        impl ModularCandle<#input_name> for #name {
            fn update(&mut self, trade: &#input_name) {
                #(
                    self.#fn_names1.update(trade);
                )*
            }

            fn reset(&mut self) {
                #(
                    self.#fn_names2.reset();
                )*
            }
        }
    };

    gen.into()
}
