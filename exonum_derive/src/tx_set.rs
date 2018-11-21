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
use syn::{Data, DataEnum, DeriveInput, Type};

struct TxSetVariant(u16, Ident, Type);

fn get_tx_variants(data: &DataEnum) -> Vec<TxSetVariant> {
    if data.variants.is_empty() {
        panic!("TransactionSet enum should not be empty");
    }

    data.variants
        .iter()
        .enumerate()
        .map(|(n, v)| {
            if v.fields.iter().len() > 1 {
                panic!("TransactionSet enum variant should have one field inside.");
            }
            let field = v
                .fields
                .iter()
                .next()
                .expect("TransactionSet enum variant can't be empty.");
            TxSetVariant(n as u16, v.ident.clone(), field.ty.clone())
        }).collect()
}

fn gen_conversions_for_transactions(
    name: &Ident,
    variants: &[TxSetVariant],
    cr: &quote::ToTokens,
) -> impl quote::ToTokens {
    let conversions = variants.iter().map(|TxSetVariant(_, id, ty)| {
        quote! {
          impl Into<#name> for #ty {
               fn into(self) -> #name {
                     #name::#id(self)
               }
          }

          impl Into<#cr::messages::ServiceTransaction> for #ty {
              fn into(self) -> #cr::messages::ServiceTransaction {
                  let set: #name = self.into();
                  set.into()
              }
          }
        }
    });

    quote!{
        #(#conversions)*
    }
}

fn gen_into_service_tx(
    name: &Ident,
    variants: &[TxSetVariant],
    cr: &quote::ToTokens,
) -> impl quote::ToTokens {
    let tx_set_impl = variants.iter().map(|TxSetVariant(n, id, _)| {
        quote! {
            #name::#id( ref tx) => ( #n, tx.encode().unwrap()),
        }
    });

    quote! {
        impl Into<#cr::messages::ServiceTransaction> for #name {
            fn into(self) -> #cr::messages::ServiceTransaction {
                let (id, vec) = match self {
                    #( #tx_set_impl )*
                };
                #cr::messages::ServiceTransaction::from_raw_unchecked(id, vec)
            }
        }
    }
}

fn gen_transaction_set_impl(
    name: &Ident,
    variants: &[TxSetVariant],
    cr: &quote::ToTokens,
) -> impl quote::ToTokens {
    let tx_set_impl = variants.iter().map(|TxSetVariant(n, id, ty)| {
        quote! {
            #n => Ok(#name::#id(#ty::decode(&vec)?)),
        }
    });

    quote! {
        impl #cr::blockchain::TransactionSet for #name {
            fn tx_from_raw(raw: #cr::messages::RawTransaction) -> std::result::Result<Self, _EncodingError> {
                let (id, vec) = raw.service_transaction().into_raw_parts();
                match id {
                    #( #tx_set_impl )*
                    num => Err(_EncodingError::Basic(format!("Tag {} not found for enum {}.",
                                                             num, stringify!(#name)).into())),
                }
            }
        }
    }
}

fn gen_into_boxed_tx_impl(
    name: &Ident,
    variants: &[TxSetVariant],
    cr: &quote::ToTokens,
) -> impl quote::ToTokens {
    let tx_set_impl = variants.iter().map(|TxSetVariant(_, id, _)| {
        quote! {
            #name::#id(tx) => Box::new(tx),
        }
    });

    quote! {
        impl Into<Box<dyn #cr::blockchain::Transaction>> for #name {
            fn into(self) -> Box<dyn #cr::blockchain::Transaction> {
                match self {
                    #( #tx_set_impl )*
                }
            }
        }
    }
}

pub fn generate_transaction_set(input: TokenStream) -> TokenStream {
    let input: DeriveInput = syn::parse(input).unwrap();

    let name = input.ident;
    let mod_name = Ident::new(&format!("tx_set_impl_{}", name), Span::call_site());
    let data = match input.data {
        Data::Enum(x) => x,
        _ => panic!("Only for enums."),
    };

    let cr = super::get_exonum_types_prefix(&input.attrs);
    let vars = get_tx_variants(&data);

    let impl_conversions = gen_conversions_for_transactions(&name, &vars, &cr);
    let into_service_tx = gen_into_service_tx(&name, &vars, &cr);
    let tx_set_impl = gen_transaction_set_impl(&name, &vars, &cr);
    let into_boxed_tx = gen_into_boxed_tx_impl(&name, &vars, &cr);

    let expanded = quote! {
        mod #mod_name{
            use super::*;
            use #cr::encoding::Error as _EncodingError;
            use #cr::messages::BinaryForm as _BinaryForm;

            #impl_conversions

            #into_service_tx

            #tx_set_impl

            #into_boxed_tx
        }
    };

    expanded.into()
}
