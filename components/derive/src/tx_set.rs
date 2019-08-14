// Copyright 2019 The Exonum Team
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
use quote::quote;
use syn::{Attribute, Data, DataEnum, DeriveInput, Fields, Lit, Type, Variant};

use std::collections::HashSet;

use crate::get_exonum_name_value_attributes;

struct ParsedVariant {
    id: u16,
    ident: Ident,
    typ: Type,
    unboxed_type: Option<Type>,
}

impl ParsedVariant {
    fn source_type(&self) -> &Type {
        self.unboxed_type.as_ref().unwrap_or(&self.typ)
    }
}

fn get_tx_variants(data: &DataEnum) -> Vec<ParsedVariant> {
    if data.variants.is_empty() {
        panic!("TransactionSet enum should not be empty");
    }

    let mut next_id = 0;
    let mut message_ids = HashSet::new();
    data.variants
        .iter()
        .map(|variant| {
            let mut fields = match &variant.fields {
                Fields::Unnamed(field) => field.unnamed.iter(),
                _ => panic!("Only unnamed fields are supported for `TransactionSet` enum."),
            };
            if fields.len() != 1 {
                panic!("Each `TransactionSet` enum variant should have one field inside.");
            }
            let field = fields.next().unwrap();

            let message_id = get_message_id(&variant).unwrap_or(next_id);
            next_id = message_id + 1;
            if message_ids.contains(&message_id) {
                panic!("Duplicate message identifier: {}", message_id);
            }
            message_ids.insert(message_id);

            ParsedVariant {
                id: message_id,
                ident: variant.ident.clone(),
                typ: field.ty.clone(),
                unboxed_type: extract_boxed_type(&field.ty),
            }
        })
        .collect()
}

fn get_message_id(variant: &Variant) -> Option<u16> {
    let literal = get_exonum_name_value_attributes(&variant.attrs)
        .iter()
        .cloned()
        .filter_map(|meta| {
            if meta.path.is_ident("message_id") {
                Some(meta.lit)
            } else {
                None
            }
        })
        .next()?;

    Some(match literal {
        Lit::Int(int_value) => int_value
            .base10_parse::<u16>()
            .expect("Cannot parse `message_id` integer value"),
        Lit::Str(str_value) => str_value
            .value()
            .parse()
            .expect("Cannot parse `message_id` expression"),
        _ => panic!("Invalid `message_id` specification, should be a `u16` constant"),
    })
}

fn extract_boxed_type(ty: &Type) -> Option<Type> {
    use syn::{
        AngleBracketedGenericArguments as Args, GenericArgument, Path, PathArguments, TypePath,
    };

    if let Type::Path(TypePath {
        path: Path { segments, .. },
        ..
    }) = ty
    {
        if segments.len() == 1 && segments[0].ident == "Box" {
            if let PathArguments::AngleBracketed(Args { ref args, .. }) = segments[0].arguments {
                if args.len() == 1 {
                    if let GenericArgument::Type(ref inner_ty) = args[0] {
                        return Some(inner_ty.clone());
                    }
                }
            }
        }
    }
    None
}

fn implement_conversions_for_enum(
    name: &Ident,
    variants: &[ParsedVariant],
) -> impl quote::ToTokens {
    let conversions = variants.iter().map(|variant| {
        let ident = &variant.ident;
        let source_type = variant.source_type();
        let constructed_variant = if variant.unboxed_type.is_some() {
            quote!(#name::#ident(Box::new(value)))
        } else {
            quote!(#name::#ident(value))
        };

        quote! {
            impl From<#source_type> for #name {
                fn from(value: #source_type) -> Self {
                    #constructed_variant
                }
            }
        }
    });

    quote! {
        #(#conversions)*
    }
}

fn implement_conversions_for_variants(
    name: &Ident,
    variants: &[ParsedVariant],
    crate_name: &impl quote::ToTokens,
) -> impl quote::ToTokens {
    let conversions = variants.iter().map(|variant| {
        let source_type = variant.source_type();
        quote! {
            impl From<#source_type> for #crate_name::messages::ServiceTransaction {
                fn from(value: #source_type) -> Self {
                    let set: #name = #name::from(value);
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
    variants: &[ParsedVariant],
    cr: &impl quote::ToTokens,
) -> impl quote::ToTokens {
    let tx_set_impl = variants.iter().map(|ParsedVariant { id, ident, .. }| {
        quote! {
            #name::#ident(ref tx) => (#id, tx.to_bytes()),
        }
    });

    quote! {
        impl From<#name> for #cr::messages::ServiceTransaction {
            fn from(value: #name) -> Self {
                let (id, vec) = match value {
                    #( #tx_set_impl )*
                };
                #cr::messages::ServiceTransaction::from_raw_unchecked(id, vec)
            }
        }
    }
}

fn implement_transaction_set_trait(
    name: &Ident,
    variants: &[ParsedVariant],
    cr: &impl quote::ToTokens,
) -> impl quote::ToTokens {
    let tx_set_impl = variants.iter().map(|variant| {
        let id = variant.id;
        let source_type = variant.source_type();
        quote! {
            #id => #source_type::from_bytes(std::borrow::Cow::from(&vec)).map(#name::from),
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
    variants: &[ParsedVariant],
    cr: &impl quote::ToTokens,
) -> impl quote::ToTokens {
    let tx_set_impl = variants.iter().map(|variant| {
        let ident = &variant.ident;
        if variant.unboxed_type.is_some() {
            quote!(#name::#ident(tx) => tx as Box<dyn #cr::blockchain::Transaction>)
        } else {
            quote!(#name::#ident(tx) => Box::new(tx))
        }
    });

    quote! {
        impl From<#name> for Box<dyn #cr::blockchain::Transaction> {
            fn from(value: #name) -> Self {
                match value {
                    #( #tx_set_impl, )*
                }
            }
        }
    }
}

fn should_convert_variants(attrs: &[Attribute]) -> bool {
    let value = get_exonum_name_value_attributes(attrs)
        .iter()
        .cloned()
        .filter_map(|meta| {
            if meta.path.is_ident("convert_variants") {
                Some(meta.lit)
            } else {
                None
            }
        })
        .next();
    if let Some(value) = value {
        match value {
            Lit::Bool(bool_value) => bool_value.value,
            Lit::Str(str_value) => str_value
                .value()
                .parse()
                .expect("Cannot parse `convert_variants` value from string"),
            _ => panic!("Invalid value type for `convert_variants` attribute"),
        }
    } else {
        // Default value is `true`.
        true
    }
}

pub fn implement_transaction_set(input: TokenStream) -> TokenStream {
    let input: DeriveInput = syn::parse(input).unwrap();

    let name = input.ident;
    let mod_name = Ident::new(&format!("tx_set_impl_{}", name), Span::call_site());
    let data = match input.data {
        Data::Enum(enum_data) => enum_data,
        _ => panic!("Only for enums."),
    };

    let crate_name = super::get_exonum_types_prefix(&input.attrs);
    let vars = get_tx_variants(&data);
    let convert_variants = should_convert_variants(&input.attrs);

    let tx_set = implement_transaction_set_trait(&name, &vars, &crate_name);
    let conversions = implement_conversions_for_enum(&name, &vars);
    let variant_conversions = if convert_variants {
        Some(implement_conversions_for_variants(
            &name,
            &vars,
            &crate_name,
        ))
    } else {
        None
    };
    let into_service_tx = implement_into_service_tx(&name, &vars, &crate_name);
    let into_boxed_tx = implement_into_boxed_tx(&name, &vars, &crate_name);

    let expanded = quote! {
        mod #mod_name{
            extern crate failure as _failure;

            use super::*;
            use self::_failure::{bail, Error as _FailureError};
            use exonum_merkledb::BinaryValue as _BinaryValue;

            #conversions
            #variant_conversions
            #tx_set
            #into_service_tx
            #into_boxed_tx
        }
    };

    expanded.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn get_message_id_works() {
        let transaction_set: DeriveInput = parse_quote! {
            pub enum Transactions {
                Issue(Issue),

                #[exonum(other_attribute = "foo")]
                Transfer(Transfer),

                /// Doc comment.
                #[exonum(message_id = 5, other_attribute = "bar")]
                Lock(Lock),

                /// Another doc comment.
                #[serde(name = "unlocking")]
                #[exonum(message_id = "10")]
                Unlock(Unlock),
            }
        };
        let transaction_set = match transaction_set.data {
            Data::Enum(enum_data) => enum_data,
            _ => unreachable!(),
        };

        assert!(get_message_id(&transaction_set.variants[0]).is_none());
        assert!(get_message_id(&transaction_set.variants[1]).is_none());
        assert_eq!(get_message_id(&transaction_set.variants[2]), Some(5));
        assert_eq!(get_message_id(&transaction_set.variants[3]), Some(10));
    }

    #[test]
    fn message_ids_assign_automatically() {
        let transaction_set: DeriveInput = parse_quote! {
            pub enum Transactions {
                Issue(Issue),
                Transfer(Transfer),

                #[exonum(message_id = 10)]
                Lock(Lock),
                Unlock(Unlock),
            }
        };
        let transaction_set = match transaction_set.data {
            Data::Enum(enum_data) => enum_data,
            _ => unreachable!(),
        };

        let variants = get_tx_variants(&transaction_set);
        assert_eq!(variants.len(), 4);
        assert_eq!(variants[0].id, 0);
        assert_eq!(variants[1].id, 1);
        assert_eq!(variants[2].id, 10);
        assert_eq!(variants[3].id, 11);
    }

    #[test]
    fn should_convert_variants_works() {
        let transaction_set: DeriveInput = parse_quote! {
            pub enum Transactions {
                Issue(Issue),
                Transfer(Transfer),
            }
        };
        assert!(should_convert_variants(&transaction_set.attrs));

        let transaction_set: DeriveInput = parse_quote! {
            #[exonum(convert_variants = "false")]
            pub enum Transactions {
                Issue(Issue),
                Transfer(Transfer),
            }
        };
        assert!(!should_convert_variants(&transaction_set.attrs));

        let transaction_set: DeriveInput = parse_quote! {
            #[exonum(convert_variants = false)]
            pub enum Transactions {
                Issue(Issue),
                Transfer(Transfer),
            }
        };
        assert!(!should_convert_variants(&transaction_set.attrs));
    }

    #[test]
    fn extract_boxed_type_works() {
        let ty: Type = parse_quote!(Box<Foo>);
        assert!(extract_boxed_type(&ty).is_some());
        let ty: Type = parse_quote!(Foo);
        assert!(extract_boxed_type(&ty).is_none());
        let ty: Type = parse_quote!(Box<Foo, Bar>);
        assert!(extract_boxed_type(&ty).is_none());
    }
}
