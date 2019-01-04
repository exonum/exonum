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
use syn::{Data, DataEnum, DeriveInput, Fields, Type};

struct Variant {
    id: u16,
    ident: Ident,
    typ: Type,
}

fn get_tx_variants(data: &DataEnum) -> Vec<Variant> {
    if data.variants.is_empty() {
        panic!("TransactionSet enum should not be empty");
    }

    data.variants
        .iter()
        .enumerate()
        .map(|(n, v)| {
            let mut fields = match &v.fields {
                Fields::Unnamed(f) => f.unnamed.iter(),
                _ => panic!("Only unnamed fields are supported for TransactionSet enum"),
            };
            if fields.len() != 1 {
                panic!("TransactionSet enum variant should have one field inside.");
            }
            let field = fields.next().unwrap();
            Variant {
                id: n as u16,
                ident: v.ident.clone(),
                typ: field.ty.clone(),
            }
        })
        .collect()
}

fn implement_conversions_for_transactions(
    name: &Ident,
    variants: &[Variant],
    cr: &quote::ToTokens,
) -> impl quote::ToTokens {
    let conversions = variants.iter().map(|Variant { ident, typ, .. }| {
        quote! {
          impl From<#typ> for #name {
               fn from(tx: #typ) -> Self {
                     #name::#ident(tx)
               }
          }

          impl Into<#cr::messages::ServiceTransaction> for #typ {
              fn into(self) -> #cr::messages::ServiceTransaction {
                  let set: #name = self.into();
                  set.into()
              }
          }
        }
    });

    quote! {
        #(#conversions)*
    }
}

fn implement_into_service_tx(
    name: &Ident,
    variants: &[Variant],
    cr: &quote::ToTokens,
) -> impl quote::ToTokens {
    let tx_set_impl = variants.iter().map(|Variant { id, ident, .. }| {
        quote! {
            #name::#ident(ref tx) => (#id, tx.encode().unwrap()),
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

fn implement_transaction_set_trait(
    name: &Ident,
    variants: &[Variant],
    cr: &quote::ToTokens,
) -> impl quote::ToTokens {
    let tx_set_impl = variants.iter().map(|Variant { id, ident, typ }| {
        quote! {
            #id => Ok(#name::#ident(#typ::decode(&vec)?)),
        }
    });

    quote! {
        impl #cr::blockchain::TransactionSet for #name {
            fn tx_from_raw(
                raw: #cr::messages::RawTransaction,
            ) -> std::result::Result<Self, _FailureError> {
                let (id, vec) = raw.service_transaction().into_raw_parts();
                match id {
                    #( #tx_set_impl )*
                    num => bail!("Tag {} not found for enum {}.", num, stringify!(#name)),
                }
            }
        }
    }
}

fn implement_into_boxed_tx(
    name: &Ident,
    variants: &[Variant],
    cr: &quote::ToTokens,
) -> impl quote::ToTokens {
    let tx_set_impl = variants.iter().map(|Variant { ident, .. }| {
        quote! {
            #name::#ident(tx) => Box::new(tx),
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

pub fn implement_transaction_set(input: TokenStream) -> TokenStream {
    let input: DeriveInput = syn::parse(input).unwrap();

    let name = input.ident;
    let mod_name = Ident::new(&format!("tx_set_impl_{}", name), Span::call_site());
    let data = match input.data {
        Data::Enum(x) => x,
        _ => panic!("Only for enums."),
    };

    let cr = super::get_exonum_types_prefix(&input.attrs);
    let vars = get_tx_variants(&data);

    let tx_set = implement_transaction_set_trait(&name, &vars, &cr);
    let conversions = implement_conversions_for_transactions(&name, &vars, &cr);
    let into_service_tx = implement_into_service_tx(&name, &vars, &cr);
    let into_boxed_tx = implement_into_boxed_tx(&name, &vars, &cr);

    let expanded = quote! {
        mod #mod_name{
            extern crate failure as _failure;

            use super::*;
            use self::_failure::{bail, Error as _FailureError};
            use #cr::messages::BinaryForm as _BinaryForm;

            #conversions
            #tx_set
            #into_service_tx
            #into_boxed_tx
        }
    };

    expanded.into()
}
