use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DataStruct, DeriveInput, Fields, FieldsNamed, FieldsUnnamed, GenericParam, Generics, parse_quote};

pub fn derive_decode_impl(input: DeriveInput) -> TokenStream {
    let name = input.ident;

    let generics = trait_bounds(input.generics);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let decode = decode(&input.data);
    quote! {
        #[automatically_derived]
        impl #impl_generics crate::emulator::abstractions::mmu::encoding::Decode for #name #ty_generics #where_clause {
            fn decode<Endianness: byteorder::ByteOrder>(buffer: &[u8]) -> Result<Self, crate::emulator::abstractions::mmu::MemoryError> {
                use crate::emulator::abstractions::mmu::encoding::RequiredMem;
                if buffer.len() != Self::required_mem() {
                    Err(crate::emulator::abstractions::mmu::MemoryError::InvalidSize)?
                }
                #decode
            }
        }
    }
}

fn trait_bounds(mut generics: Generics) -> Generics {
    for param in &mut generics.params {
        if let GenericParam::Type(ref mut type_param) = *param {
            type_param
                .bounds
                .push(parse_quote!(fuzzyemu::emulator::abstractions::mmu::encoding::Decode));
        }
    }
    generics
}

fn decode(data: &Data) -> TokenStream {
    match data {
        Data::Struct(s) => decode_struct(s),
        Data::Enum(_) => unimplemented!(),
        Data::Union(_) => unimplemented!(),
    }
}

fn offsets(data: &DataStruct) -> TokenStream {
    let offsets = data.fields.iter().map(|field| {
        let field_type = &field.ty;
        quote! {
            #field_type::required_mem(),
        }
    }).collect::<TokenStream>();
    quote! {
        let offsets = [0, #offsets].into_iter().scan(0, |sum, i| {*sum += i; Some(*sum)}).collect::<Vec<_>>();
    }
}

fn decode_struct(data: &DataStruct) -> TokenStream {
    let offsets = offsets(data);
    let fields = match &data.fields {
        Fields::Named(fields) => decode_named(fields),
        Fields::Unnamed(fields) => decode_unnamed(fields),
        Fields::Unit => unimplemented!(),
    };
    quote! {
        #offsets
        #fields
    }
}

fn decode_named(fields: &FieldsNamed) -> TokenStream {
    let fields = fields.named.iter().enumerate().map(|(i, field)| {
        let field_name = field.ident.as_ref().unwrap();
        let field_type = &field.ty;
        quote! {
            #field_name: #field_type::decode::<Endianness>(&buffer[offsets[#i]..offsets[#i+1]])?,
        }
    }).collect::<TokenStream>();
    quote! {
        Ok(Self {
            #fields
        })
    }
}

fn decode_unnamed(fields: &FieldsUnnamed) -> TokenStream {
    let fields = fields.unnamed.iter().enumerate().map(|(i, field)| {
        let field_type = &field.ty;
        quote! {
            #field_type::decode::<Endianness>(&buffer[offsets[#i]..offsets[#i+1]])?,
        }
    }).collect::<TokenStream>();
    quote! {
        Ok(Self(#fields))
    }
}