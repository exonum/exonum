use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use syn::{Data, DeriveInput};

pub fn generate_transaction_set(input: TokenStream) -> TokenStream {
    let input: DeriveInput = syn::parse(input).unwrap();

    let name = input.ident;
    let mod_name = Ident::new(&format!("tx_set_impl_{}", name), Span::call_site());
    let data = match input.data {
        Data::Enum(x) => x,
        _ => panic!("Only for enums"),
    };

    let cr = super::get_exonum_types_prefix(&input.attrs);

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
            (n as u16, v.ident.clone(), field.ty.clone())
        }).collect::<Vec<_>>();

    let convert_1 = variants.iter().map(|(_, id, ty)| {
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

    let into_service_tx = {
        let tx_set_impls = variants.iter().map(|(n, id, _)| {
            quote! {
                #name::#id( ref tx) => ( #n, tx.encode().unwrap()),
            }
        });

        quote! {
            impl Into<#cr::messages::ServiceTransaction> for #name {
                fn into(self) -> #cr::messages::ServiceTransaction {
                    let (id, vec) = match self {
                        #( #tx_set_impls )*
                    };
                    #cr::messages::ServiceTransaction::from_raw_unchecked(id, vec)
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
            impl #cr::blockchain::TransactionSet for #name {
                fn tx_from_raw(raw: #cr::messages::RawTransaction) -> Result<Self, _EncodingError> {
                    let (id, vec) = raw.service_transaction().into_raw_parts();
                    match id {
                        #( #tx_set_impls )*
                        num => Err(_EncodingError::Basic(
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
            impl Into<Box<dyn #cr::blockchain::Transaction>> for #name {
                fn into(self) -> Box<dyn #cr::blockchain::Transaction> {
                    match self {
                        #( #tx_set_impls )*
                    }
                }
            }
        }
    };

    let expanded = quote! {
        mod #mod_name{
            use super::*;
            use #cr::encoding::Error as _EncodingError;
            use #cr::messages::BinaryForm as _BinaryForm;

            #(#convert_1)*

            #into_service_tx

            #tx_set_impl

            #into_boxed_tx
        }
    };

    expanded.into()
}
