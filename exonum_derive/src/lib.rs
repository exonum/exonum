#![recursion_limit = "128"]

extern crate proc_macro;
extern crate proc_macro2;

extern crate syn;
#[macro_use]
extern crate quote;

use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use syn::{Data, DeriveInput};

#[proc_macro_derive(ProtobufConvert)]
pub fn protobuf_convert_derive(input: TokenStream) -> TokenStream {
    let input: DeriveInput = syn::parse(input).unwrap();

    let name = input.ident;
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
        let names1 = field_names.clone();
        let setters = names1
            .iter()
            .map(|i| Ident::new(&format!("set_{}", i), Span::call_site()));
        let names2 = field_names.clone();
        quote! {
            fn to_pb(&self) -> Self::ProtoStruct {
                let mut msg = Self::ProtoStruct::new();
                #( msg.#setters(self.#names2.to_pb().into()); )*
                msg
            }
        }
    };

    let from_pb_fn = {
        let names1 = field_names.clone();
        let getters = names1
            .iter()
            .map(|i| Ident::new(&format!("get_{}", i), Span::call_site()));
        let names2 = field_names.clone();
        quote! {
            fn from_pb(pb: Self::ProtoStruct) -> Result<Self, ()> {
              Ok(Self {
               #( #names2: ProtobufConvert::from_pb(pb.#getters().to_owned())?, )*
              })
            }
        }
    };

    let expanded = quote! {
        impl ProtobufConvert for #name {
            type ProtoStruct = protobuf::#name;

            #to_pb_fn

            #from_pb_fn

        }

        impl BinaryForm for #name
        {
            fn encode(&self) -> Result<Vec<u8>, encoding::Error> {
                Ok(self.to_pb().write_to_bytes().unwrap())
            }

            fn decode(buffer: &[u8]) -> Result<Self, encoding::Error> {
                let mut pb = <Self as ProtobufConvert>::ProtoStruct::new();
                pb.merge_from_bytes(buffer).unwrap();
                Self::from_pb(pb).map_err(|_| "Conversion from protobuf error".into())
            }
        }

        impl CryptoHash for #name {
            fn hash(&self) -> Hash {
                let v = self.to_pb().write_to_bytes().unwrap();
                crypto::hash(&v)
            }
        }

        impl StorageValue for #name {
            fn into_bytes(self) -> Vec<u8> {
                self.to_pb().write_to_bytes().unwrap()
            }

            fn from_bytes(value: Cow<[u8]>) -> Self {
                let mut block = <Self as ProtobufConvert>::ProtoStruct::new();
                block.merge_from_bytes(value.as_ref()).unwrap();
                ProtobufConvert::from_pb(block).unwrap()
            }
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_derive(TransactionSet)]
pub fn transaction_set_derive(input: TokenStream) -> TokenStream {
    let input: DeriveInput = syn::parse(input).unwrap();

    let name = input.ident;
    let data = match input.data {
        Data::Enum(x) => x,
        _ => panic!("Only for enums"),
    };

    if data.variants.is_empty() {
        panic!("TransactionSet enum should not be empty");
    }

    let variants = data
        .variants
        .iter()
        .enumerate()
        .map(|(n, v)| {
            if v.fields.iter().len() > 1 {
                panic!("TransactionSet enum variant should have one field inside");
            }
            let field = v
                .fields
                .iter()
                .next()
                .expect("TransactionSet enum variant can't be empty");
            (n, v.ident.clone(), field.ty.clone())
        }).collect::<Vec<_>>();

    let convert_1 = variants.iter().map(|(_, id, ty)| {
        quote! {
          impl Into<#name> for #ty {
               fn into(self) -> #name {
                     #name::#id(self)
               }
          }

          impl Into<ServiceTransaction> for #ty {
              fn into(self) -> ServiceTransaction {
                  let set: #name = self.into();
                  set.into()
              }
          }
        }
    });

    let into_service_tx = {
        let tx_set_impls = variants.iter().map(|(n, id, _)| {
            quote! {
                #name::#id(ref tx) => (#n, tx.encode().unwrap()),
            }
        });

        quote! {
            impl Into<ServiceTransaction> for #name {
                fn into(self) -> ServiceTransaction {
                    let (id, vec) = match self {
                        #( #tx_set_impls )*
                    };
                    ServiceTransaction::from_raw_unchecked(id, vec)
                }
            }
        }
    };

    let tx_set_impl = {
        let tx_set_impls = variants.iter().map(|(n, id, ty)| {
            quote! {
                #n => {
                    Ok(#name::#id(#ty::decode(&vec)?))
                },
            }
        });

        quote! {

            impl TransactionSet for #name {
                fn tx_from_raw(raw: RawTransaction) -> Result<Self, MessageError> {
                    let (id, vec) = raw.service_transaction().into_raw_parts();
                    match id {
                        #( #tx_set_impls )*
                        num => Err(MessageError::Basic(
                            format!(
                                "Tag {} not found for enum {}.",
                                num, stringify!(#name)
                            ).into(),
                        )),
                    }
                }
            }

        }
    };

    let into_boxed_tx = {
        let tx_set_impls = variants.iter().map(|(_, id, _)| {
            quote! {
                #name::#id(tx) => Box::new(tx),
            }
        });

        quote! {
            impl Into<Box<dyn Transaction>> for #name {
                fn into(self) -> Box<dyn Transaction> {
                    match self {
                        #( #tx_set_impls )*
                    }
                }
            }
        }
    };

    let expanded = quote! {
        #(#convert_1)*

        #into_service_tx

        #tx_set_impl

        #into_boxed_tx
    };

    TokenStream::from(expanded)
}
