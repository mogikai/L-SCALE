use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, GenericParam, Generics, parse_macro_input, parse_quote};

#[proc_macro_derive(RequiredMem)]
pub fn derive_required_mem(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;
    let expaned = quote! {
        impl RequiredMem for #name {}
    };
    proc_macro::TokenStream::from(expaned)
}

#[proc_macro_derive(Encode)]
pub fn derive_encode(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let generics = add_trait_bounds(input.generics);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let encode = encode(&input.data);
    let expanded = quote! {
        #[automatically_derived]
        impl #impl_generics crate::emulator::abstractions::mmu::encoding::Encode for #name #ty_generics #where_clause {
            fn encode<Endianness: ByteOrder>(&self, buffer: &mut Vec<u8>) {
                buffer.reserve(Self::required_mem());
                #encode
            }
        }
    };
    proc_macro::TokenStream::from(expanded)
}

fn add_trait_bounds(mut generics: Generics) -> Generics {
    for param in &mut generics.params {
        if let GenericParam::Type(ref mut type_param) = *param {
            type_param
                .bounds
                .push(parse_quote!(fuzzyemu::emulator::abstractions::mmu::encoding::Encode));
        }
    }
    generics
}

fn encode(data: &Data) -> TokenStream {
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
                fields.unnamed.iter().enumerate().map(|(i, field)| {
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
