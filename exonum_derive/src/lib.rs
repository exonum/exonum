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

#![recursion_limit = "256"]

extern crate proc_macro;
extern crate proc_macro2;

extern crate syn;
#[macro_use]
extern crate quote;

mod pb_convert;
mod tx_set;

use proc_macro::TokenStream;
use syn::Attribute;

use std::env;

#[proc_macro_derive(
    ProtobufConvert,
    attributes(protobuf_convert, exonum_derive_outer)
)]
pub fn generate_protobuf_convert(input: TokenStream) -> TokenStream {
    pb_convert::generate_protobuf_convert(input)
}

#[proc_macro_derive(TransactionSet, attributes(exonum_derive_outer))]
pub fn transaction_set_derive(input: TokenStream) -> TokenStream {
    tx_set::generate_transaction_set(input)
}

fn get_exonum_types_prefix(attrs: &[Attribute]) -> impl quote::ToTokens {
    let inside_crate = {
        let derive_outer = attrs.iter().any(|attr| {
            let meta = match attr.parse_meta() {
                Ok(m) => m,
                Err(_) => return false,
            };
            meta.name() == "exonum_derive_outer"
        });

        if derive_outer {
            false
        } else {
            env::var("CARGO_PKG_NAME").unwrap() == "exonum"
        }
    };

    if inside_crate {
        quote!(crate)
    } else {
        quote!(exonum)
    }
}
