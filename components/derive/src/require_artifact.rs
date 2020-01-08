// Copyright 2020 The Exonum Team
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

use darling::FromMeta;
use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::{quote, ToTokens};
use semver::VersionReq;

use crate::{find_meta_attrs, MainCratePath};

#[derive(Debug)]
struct RequireArtifact<'a> {
    ident: &'a Ident,
    generics: &'a syn::Generics,
    attrs: RequireArtifactAttrs,
}

#[derive(Debug, Default, FromMeta)]
struct RequireArtifactAttrs {
    #[darling(rename = "crate", default)]
    cr: MainCratePath,
    #[darling(default)]
    name: Option<String>,
    #[darling(default)]
    version: Option<String>,
}

impl<'a> RequireArtifact<'a> {
    fn from_derive_input(input: &'a syn::DeriveInput) -> darling::Result<Self> {
        let meta = find_meta_attrs("require_artifact", &input.attrs);
        let attrs = meta
            .as_ref()
            .map(RequireArtifactAttrs::from_nested_meta)
            .unwrap_or_else(|| Ok(RequireArtifactAttrs::default()))?;

        if let Some(ref version) = attrs.version {
            version
                .parse::<VersionReq>()
                .map_err(|e| darling::Error::custom(e).with_span(meta.as_ref().unwrap()))?;
        }
        Ok(Self {
            ident: &input.ident,
            generics: &input.generics,
            attrs,
        })
    }

    fn module(&self) -> impl ToTokens {
        let cr = &self.attrs.cr;
        quote!(#cr::runtime::versioning)
    }

    fn required_artifact_fn(&self) -> impl ToTokens {
        let name = if let Some(ref name) = self.attrs.name {
            quote!(#name)
        } else {
            quote!(env!("CARGO_PKG_NAME"))
        };

        let version_req = if let Some(ref req) = self.attrs.version {
            quote!(#req)
        } else {
            quote!(env!("CARGO_PKG_VERSION"))
        };

        let module = self.module();
        quote! {
            #module::ArtifactReq::new(#name, #version_req.parse().unwrap())
        }
    }
}

impl ToTokens for RequireArtifact<'_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let name = self.ident;
        let (impl_generics, ty_generics, where_clause) = self.generics.split_for_impl();
        let module = self.module();
        let fn_impl = self.required_artifact_fn();

        let expanded = quote! {
            impl #impl_generics #module::RequireArtifact for #name #ty_generics #where_clause {
                fn required_artifact() -> #module::ArtifactReq {
                    #fn_impl
                }
            }
        };
        tokens.extend(expanded);
    }
}

pub fn impl_require_artifact(input: TokenStream) -> TokenStream {
    let input: syn::DeriveInput = syn::parse(input).unwrap();
    let require_artifact = match RequireArtifact::from_derive_input(&input) {
        Ok(tokens) => tokens,
        Err(e) => return e.write_errors().into(),
    };
    let tokens = quote!(#require_artifact);
    tokens.into()
}
