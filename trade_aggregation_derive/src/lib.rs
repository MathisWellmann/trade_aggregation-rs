use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{self, Data, DataStruct, Fields};

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
        _ => panic!("Use a struct"),
    };

    let fn_names = components
        .iter()
        .map(|v| v.ident.clone().unwrap().to_string().to_lowercase());
    let fn_inner = components.iter().map(|v| v.ty.clone());

    let gen = quote! {
        impl #name {
            #(
                fn #fn_names(&self) -> f64 {
                    self.#fn_names.value()
                }
            )*
        }
    };
    println!("gen: {:?}", gen);

    gen.into()
}
