use quote::quote;
use syn::{DeriveInput, parse_macro_input};

use crate::decode::derive_decode_impl;
use crate::encode::{add_trait_bounds, encode};

mod encode;
mod decode;

#[proc_macro_derive(RequiredMem)]
pub fn derive_required_mem(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;
    let expaned = quote! {
        impl crate::emulator::abstractions::mmu::encoding::RequiredMem for #name {}
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
            fn encode<Endianness: byteorder::ByteOrder>(&self, buffer: &mut Vec<u8>) {
                use crate::emulator::abstractions::mmu::encoding::RequiredMem;
                buffer.reserve(Self::required_mem());
                #encode
            }
        }
    };
    proc_macro::TokenStream::from(expanded)
}

#[proc_macro_derive(Decode)]
pub fn derive_decode(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    proc_macro::TokenStream::from(derive_decode_impl(input))
}