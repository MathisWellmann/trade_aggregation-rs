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

use proc_macro::TokenStream;
use quote::quote;
use syn::{self, Data, DataStruct, Fields};

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

fn impl_candle_macro(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let components = match &ast.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => &fields.named,
        _ => panic!("Use a named struct"),
    };

    let fn_names0 = components.iter().map(|v| v.ident.clone().unwrap());
    let fn_names1 = fn_names0.clone();
    let fn_names2 = fn_names1.clone();

    let gen = quote! {
        impl #name {
            #(
                pub fn #fn_names0(&self) -> f64 {
                    self.#fn_names0.value()
                }
            )*
        }

        impl ModularCandle<T> for #name {
            fn update(&mut self, trade: &T) {
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
