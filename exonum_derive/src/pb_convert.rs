// Copyright 2018 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use syn::{Attribute, Data, DeriveInput, Lit, Meta, NestedMeta, Path};

fn get_protobuf_struct_path(attrs: &[Attribute]) -> Path {
    attrs
        .iter()
        .find_map(|attr| {
            let meta = attr.parse_meta().ok()?;
            if meta.name() != "protobuf_convert" {
                return None;
            }
            let list = match meta {
                Meta::List(x) => x,
                _ => panic!("protobuf_convert attribute expects one argument"),
            };
            let name: Path = match list.nested.iter().next().expect("h") {
                NestedMeta::Literal(Lit::Str(lit_str)) => lit_str
                    .parse()
                    .expect("protobuf_convert argument should be a valid type path"),
                _ => panic!("protobuf_convert argument should be a string"),
            };
            Some(name)
        }).expect("protobuf_convert attribute is not set")
}

pub fn generate_protobuf_convert(input: TokenStream) -> TokenStream {
    let input: DeriveInput = syn::parse(input).unwrap();

    let name = input.ident;
    let proto_struct_name = get_protobuf_struct_path(&input.attrs);
    let cr = super::get_exonum_types_prefix(&input.attrs);

    let mod_name = Ident::new(&format!("pb_convert_impl_{}", name), Span::call_site());
    let data = match input.data {
        Data::Struct(x) => x,
        _ => panic!("Protobuf convert can be derived for structs only"),
    };

    let field_names = data
        .fields
        .iter()
        .map(|f| f.ident.clone().unwrap())
        .collect::<Vec<_>>();

    let to_pb_fn = {
        let setters = field_names
            .iter()
            .map(|i| Ident::new(&format!("set_{}", i), Span::call_site()));
        let our_struct_names = field_names.clone();
        quote! {
            fn to_pb(&self) -> Self::ProtoStruct {
                let mut msg = Self::ProtoStruct::new();
                #( msg.#setters(ProtobufConvert::to_pb(&self.#our_struct_names).into()); )*
                msg
            }
        }
    };

    let from_pb_fn = {
        let getters = field_names
            .iter()
            .map(|i| Ident::new(&format!("get_{}", i), Span::call_site()));
        let our_struct_names = field_names.clone();
        quote! {
            fn from_pb(pb: Self::ProtoStruct) -> Result<Self, ()> {
              Ok(Self {
               #( #our_struct_names: ProtobufConvert::from_pb(pb.#getters().to_owned())?, )*
              })
            }
        }
    };
    let expanded = quote! {
        mod #mod_name {
            extern crate protobuf as _protobuf_crate;

            use super::*;

            use #cr::encoding::protobuf::ProtobufConvert;
            use #cr::encoding::Error as _EncodingError;
            use self::_protobuf_crate::Message as _ProtobufMessage;

            impl ProtobufConvert for #name {
                type ProtoStruct = #proto_struct_name;

                #to_pb_fn

                #from_pb_fn

            }

            impl #cr::messages::BinaryForm for #name
            {
                fn encode(&self) -> std::result::Result<Vec<u8>, _EncodingError> {
                    Ok(self.to_pb().write_to_bytes().unwrap())
                }

                fn decode(buffer: &[u8]) -> std::result::Result<Self, _EncodingError> {
                    let mut pb = <Self as ProtobufConvert>::ProtoStruct::new();
                    pb.merge_from_bytes(buffer).unwrap();
                    Self::from_pb(pb).map_err(|_| "Conversion from protobuf error".into())
                }
            }

            impl #cr::crypto::CryptoHash for #name {
                fn hash(&self) -> #cr::crypto::Hash {
                    let v = self.to_pb().write_to_bytes().unwrap();
                    #cr::crypto::hash(&v)
                }
            }

            impl #cr::storage::StorageValue for #name {
                fn into_bytes(self) -> Vec<u8> {
                    self.to_pb().write_to_bytes().unwrap()
                }

                fn from_bytes(value: std::borrow::Cow<[u8]>) -> Self {
                    let mut block = <Self as ProtobufConvert>::ProtoStruct::new();
                    block.merge_from_bytes(value.as_ref()).unwrap();
                    ProtobufConvert::from_pb(block).unwrap()
                }
            }
        }
    };

    expanded.into()
}
