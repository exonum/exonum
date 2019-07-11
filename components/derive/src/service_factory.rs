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
use quote::quote;
use syn::{DeriveInput, Lit, Meta, Path};

use super::{
    get_exonum_name_value_attributes, get_exonum_types_prefix, ARTIFACT_NAME, ARTIFACT_VERSION,
    PROTO_SOURCES,
};

fn find_exonum_literal_attribute(meta_attrs: &[Meta], ident_name: &str) -> Option<Lit> {
    let map_attrs = get_exonum_name_value_attributes(meta_attrs);
    map_attrs.into_iter().find_map(|nv| {
        if nv.ident == ident_name {
            Some(nv.lit)
        } else {
            None
        }
    })
}

fn get_protobuf_sources_mod_path(meta_attrs: &[Meta]) -> Path {
    let map_attrs = get_exonum_name_value_attributes(meta_attrs);
    let struct_path = map_attrs.into_iter().find_map(|nv| {
        if nv.ident == PROTO_SOURCES {
            match nv.lit {
                Lit::Str(path) => Some(path.parse::<Path>().unwrap()),
                _ => None,
            }
        } else {
            None
        }
    });
    struct_path.unwrap_or_else(|| panic!("{} attribute is not set properly.", PROTO_SOURCES))
}

macro_rules! literal_or_default {
    ($meta_attrs:expr, $name:expr, $default:expr) => {
        if let Some(lit) = find_exonum_literal_attribute($meta_attrs, $name) {
            quote! { #lit }
        } else {
            quote! { $default }
        };
    };
}

pub fn implement_service_factory(input: TokenStream) -> TokenStream {
    let input: DeriveInput = syn::parse(input).unwrap();
    let meta_attrs = input
        .attrs
        .iter()
        .filter_map(|a| a.parse_meta().ok())
        .collect::<Vec<_>>();

    let name = input.ident.clone();
    let cr = get_exonum_types_prefix(&meta_attrs);
    let proto_sources_mod = get_protobuf_sources_mod_path(&meta_attrs);

    let artifact_name = literal_or_default!(&meta_attrs, ARTIFACT_NAME, env!("CARGO_PKG_NAME"));
    let artifact_version =
        literal_or_default!(&meta_attrs, ARTIFACT_VERSION, env!("CARGO_PKG_VERSION"));

    let expanded = quote! {
        impl #cr::runtime::rust::ServiceFactory for #name {
            fn artifact_id(&self) -> #cr::runtime::rust::RustArtifactId {
                concat!(#artifact_name, "/", #artifact_version).parse().unwrap()
            }

            fn artifact_info(&self) -> #cr::runtime::ArtifactInfo {
                #cr::runtime::ArtifactInfo {
                    proto_sources: #proto_sources_mod::PROTO_SOURCES.as_ref(),
                }
            }

            fn create_instance(&self) -> Box<dyn #cr::runtime::rust::Service> {
                Box::new(Self)
            }
        }
    };

    expanded.into()
}
