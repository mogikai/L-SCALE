use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, GenericParam, Generics, parse_macro_input, parse_quote};

pub fn add_trait_bounds(mut generics: Generics) -> Generics {
    for param in &mut generics.params {
        if let GenericParam::Type(ref mut type_param) = *param {
            type_param
                .bounds
                .push(parse_quote!(fuzzyemu::emulator::abstractions::mmu::encoding::Encode));
        }
    }
    generics
}

pub fn encode(data: &Data) -> TokenStream {
    match *data {
        Data::Struct(ref data) => match &data.fields {
            Fields::Named(fields) => {
                fields.named.iter().map(|field| {
                    let field_name = field.ident.as_ref().unwrap();
                    quote! {
                        self.#field_name.encode::<Endianness>(buffer);
                    }
                }).collect::<TokenStream>()
            }
            Fields::Unnamed(fields) => {
                fields.unnamed.iter().enumerate().map(|(i, _)| {
                    let i = syn::Index::from(i);
                    quote! {
                        self.#i.encode::<Endianness>(buffer);
                    }
                }).collect::<TokenStream>()
            }
            Fields::Unit => {
                quote! {}
            }
        },
        Data::Enum(_) => unimplemented!(),
        Data::Union(_) => unimplemented!(),
    }
}
