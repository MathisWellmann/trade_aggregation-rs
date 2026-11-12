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
//! The generated code uses absolute paths (`::trade_aggregation::...`),
//! so nothing has to be in scope where the macro is used.

#![deny(missing_docs)]

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, punctuated::Punctuated, Data, DataStruct, DeriveInput, Field, Fields, Ident,
    Path, Token, Type,
};

/// The 'Candle' macro takes a named struct,
/// that has multiple fields of type 'CandleComponent'
/// to automatically generate a struct that implements
/// the 'ModularCandle' trait for every event type that all components accept,
/// which means it can then be used in the aggregation process.
/// It also exposes getter functions for each 'CandleComponent' for convenience,
/// and a `FIELDS` constant with the names of the components.
///
/// Fields of type `PhantomData` are not components and are ignored.
///
/// `#[candle(record = MyRecord)]` also generates a struct `MyRecord` with the value of each component
/// as a public field, and a method `record(&self) -> MyRecord`, e.g. to serialize a closed candle.
/// `#[candle(record_derive(Debug, Clone, serde::Serialize))]` adds derives to that struct.
#[proc_macro_derive(Candle, attributes(candle))]
pub fn candle_macro_derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_candle_macro(&ast)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// The options of the `#[candle(...)]` attribute.
#[derive(Default)]
struct Options {
    record: Option<Ident>,
    record_derives: Vec<Path>,
}

fn parse_options(ast: &DeriveInput) -> syn::Result<Options> {
    let mut options = Options::default();
    for attr in ast.attrs.iter().filter(|a| a.path().is_ident("candle")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("record") {
                options.record = Some(meta.value()?.parse()?);
                return Ok(());
            }
            if meta.path.is_ident("record_derive") {
                let content;
                syn::parenthesized!(content in meta.input);
                let paths = Punctuated::<Path, Token![,]>::parse_terminated(&content)?;
                options.record_derives.extend(paths);
                return Ok(());
            }
            Err(meta.error("expected `record = Name` or `record_derive(...)`"))
        })?;
    }
    if options.record.is_none() && !options.record_derives.is_empty() {
        return Err(syn::Error::new_spanned(
            &ast.ident,
            "`record_derive` needs `record = Name`",
        ));
    }
    Ok(options)
}

/// Whether the field is a `PhantomData`, e.g. the `input` marker of older versions.
fn is_phantom(field: &Field) -> bool {
    match &field.ty {
        Type::Path(p) => p
            .path
            .segments
            .last()
            .is_some_and(|s| s.ident == "PhantomData"),
        _ => false,
    }
}

fn impl_candle_macro(ast: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &ast.ident;
    let vis = &ast.vis;
    let fields = match &ast.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => &fields.named,
        _ => {
            return Err(syn::Error::new_spanned(
                &ast.ident,
                "`Candle` needs a struct with named fields",
            ))
        }
    };
    let options = parse_options(ast)?;

    let components: Vec<&Field> = fields.iter().filter(|f| !is_phantom(f)).collect();
    let idents: Vec<&Ident> = components
        .iter()
        .map(|f| f.ident.as_ref().expect("named fields have identifiers"))
        .collect();
    let types: Vec<&Type> = components.iter().map(|f| &f.ty).collect();
    let names: Vec<String> = idents.iter().map(|i| i.to_string()).collect();
    let count = names.len();

    let ta = quote!(::trade_aggregation);
    let event = format_ident!("__CandleEvent");
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    let mut event_generics = ast.generics.clone();
    event_generics.params.push(syn::parse_quote!(#event));
    let (event_impl_generics, _, _) = event_generics.split_for_impl();
    let mut event_where = where_clause
        .cloned()
        .unwrap_or_else(|| syn::parse_quote!(where));
    for ty in &types {
        event_where
            .predicates
            .push(syn::parse_quote!(#ty: #ta::CandleComponentUpdate<#event>));
    }

    let record = match &options.record {
        None => quote!(),
        Some(record) => {
            if !ast.generics.params.is_empty() {
                return Err(syn::Error::new_spanned(
                    &ast.generics,
                    "`record` needs a candle without generic parameters",
                ));
            }
            let derives = &options.record_derives;
            quote! {
                /// The values of the components of a candle.
                #[derive(#(#derives),*)]
                #vis struct #record {
                    #(
                        #[doc = concat!("The value of `", #names, "`.")]
                        pub #idents: <#types as #ta::CandleComponent>::Value,
                    )*
                }

                impl #name {
                    /// The values of all components.
                    pub fn record(&self) -> #record {
                        #record {
                            #( #idents: self.#idents(), )*
                        }
                    }
                }
            }
        }
    };

    Ok(quote! {
        impl #impl_generics #name #ty_generics #where_clause {
            /// The names of the components, in the order of the fields.
            pub const FIELDS: [&'static str; #count] = [#(#names),*];

            #(
                /// Get the value of this candle component.
                pub fn #idents(&self) -> <#types as #ta::CandleComponent>::Value {
                    #ta::CandleComponent::value(&self.#idents)
                }
            )*
        }

        impl #event_impl_generics #ta::ModularCandle<#event> for #name #ty_generics #event_where {
            fn update(&mut self, event: &#event) {
                #(
                    #ta::CandleComponentUpdate::<#event>::update(&mut self.#idents, event);
                )*
            }

            fn reset(&mut self) {
                #(
                    #ta::CandleComponent::reset(&mut self.#idents);
                )*
            }

            fn close_at(&mut self, timestamp: i64) {
                #(
                    #ta::CandleComponentUpdate::<#event>::close_at(&mut self.#idents, timestamp);
                )*
            }
        }

        #record
    })
}
