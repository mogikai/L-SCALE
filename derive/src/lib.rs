use quote::quote;
use syn::{
    parse_macro_input,
    DeriveInput,
};

use crate::decode::derive_decode_impl;
use crate::encode::{
    add_trait_bounds,
    encode,
};

mod decode;
mod encode;

#[proc_macro_derive(RequiredMem)]
pub fn derive_required_mem(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;
    let expaned = quote! {
        impl mogi_utilities::encoding::RequiredMem for #name {}
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
        impl #impl_generics mogi_utilities::encoding::Encode for #name #ty_generics #where_clause {
            fn encode<Endianness: byteorder::ByteOrder>(&self, buffer: &mut Vec<u8>) {
                use mogi_utilities::encoding::RequiredMem;
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

/// Can be used to mark code that is not feature complete and needs further implementation, but works for
/// a few or most cases. If the feature `do_not_compile_unfinished` is enabled, the macro will result in
/// a compilation failure. If the feature `panic_on_unfinished` is enabled, the macro will cause a panic,
/// once the code in question is reached.
#[proc_macro_attribute]
pub fn unfinished(
    _: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    #[cfg(feature = "panic_on_unfinished")]
    return {
        use syn::ItemFn;
        let ItemFn { attrs, vis, sig, block } = parse_macro_input!(item as ItemFn);
        let statements = &block.stmts;
        proc_macro::TokenStream::from(quote! {
            #(#attrs)* #vis #sig {
                todo!("The code in this function is unfinished and lacks some functionality");
                #(#statements)*
            }
        })
    };
    #[cfg(feature = "do_not_compile_unfinished")]
    return {
        use syn::ItemFn;
        let ItemFn { attrs, vis, sig, block } = parse_macro_input!(item as ItemFn);
        let statements = &block.stmts;
        proc_macro::TokenStream::from(quote! {
            #(#attrs)* #vis #sig {
                compile_error!("The code in this function is unfinished and lacks some functionality");
                #(#statements)*
            }
        })
    };
    #[cfg(not(any(feature = "panic_on_unfinished", feature = "do_not_compile_unfinished")))]
    return item;
}
