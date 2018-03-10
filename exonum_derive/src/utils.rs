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

use quote::{Tokens, ToTokens};
use syn::{self, TypeReference, GenericArgument,
          AngleBracketedGenericArguments as GenericArguments, Attribute, Path, PathSegment, Ident,
          Lit, Expr, Meta, MetaList, MetaNameValue, NestedMeta};
use syn::punctuated::Punctuated;
use syn::visit_mut::{self, VisitMut};
use synstructure::{BindingInfo, VariantInfo};

use std::iter::FromIterator;

struct LifetimeStripper;

impl VisitMut for LifetimeStripper {
    fn visit_type_reference_mut(&mut self, reference: &mut TypeReference) {
        reference.lifetime = None;
        visit_mut::visit_type_reference_mut(self, reference);
    }

    fn visit_angle_bracketed_generic_arguments_mut(&mut self, args: &mut GenericArguments) {
        args.args = {
            let filtered = args.args
                .iter()
                .filter(|&arg| match *arg {
                    GenericArgument::Lifetime(..) => false,
                    _ => true,
                })
                .cloned();

            Punctuated::from_iter(filtered)
        };
        visit_mut::visit_angle_bracketed_generic_arguments_mut(self, args);
    }
}

/// Strips lifetimes from the given type.
pub fn strip_lifetimes(ty: &syn::Type) -> syn::Type {
    let mut ty = ty.clone();
    LifetimeStripper.visit_type_mut(&mut ty);
    ty
}

#[test]
fn test_strip_lifetimes() {
    use quote::ToTokens;

    let ty: syn::Type = syn::parse_str("std::collections::HashMap<String, Value>").unwrap();
    assert_eq!(
        strip_lifetimes(&ty).into_tokens(),
        quote!(std::collections::HashMap<String, Value>)
    );

    let ty: syn::Type = syn::parse_str("std::slice::Iter<'a, T>").unwrap();
    assert_eq!(
        strip_lifetimes(&ty).into_tokens(),
        quote!(std::slice::Iter<T>)
    );
}

pub fn is_exonum_attr(attr: &Attribute) -> bool {
    let exonum_path = Path::from(PathSegment::from(Ident::from("exonum")));
    attr.path == exonum_path
}

#[test]
fn test_is_exonum_attr() {
    let exonum_attrs: [Attribute; 5] = [
        parse_quote!(#[exonum]),
        parse_quote!(#[exonum = "foo"]),
        parse_quote!(#[exonum ("foo")]),
        parse_quote!(#[exonum(service_id = 25)]),
        parse_quote!(#[exonum(service_id = 25, message_id = 3)]),
    ];
    for attr in &exonum_attrs {
        assert!(is_exonum_attr(attr), "False negative: {:?}", attr);
    }

    let non_exonum_attrs: [Attribute; 3] = [
        parse_quote!(#[exonu = "foo"]),
        parse_quote!(#[exonum::Message("foo")]),
        parse_quote!(#[exonum_derive]),
    ];
    for attr in &non_exonum_attrs {
        assert!(!is_exonum_attr(attr), "False positive: {:?}", attr);
    }
}

pub fn literal_to_tokens(lit: &Lit) -> Option<Tokens> {
    match *lit {
        Lit::Int(ref i) if i.value() < u64::from(u16::max_value()) => {
            let id = i.value() as u16;
            Some(quote!(#id))
        }
        Lit::Str(ref s) => {
            syn::parse_str::<Expr>(&s.value())
                .map(ToTokens::into_tokens)
                .ok()
        }
        _ => None,
    }
}

#[test]
fn test_literal_to_tokens() {
    let valid_literals: [(Lit, Tokens); 6] = [
        (parse_quote!(1), quote!(1u16)),
        (parse_quote!(0xff_fe), quote!(65534u16)),
        (parse_quote!(99), quote!(99u16)),
        (parse_quote!("FOO"), quote!(FOO)),
        (parse_quote!("FOO as u16"), quote!(FOO as u16)),
        (parse_quote!("other::FOO"), quote!(other::FOO)),
    ];
    for &(ref lit, ref tokens) in &valid_literals {
        assert_eq!(literal_to_tokens(lit).unwrap(), *tokens);
    }

    let invalid_literals: [Lit; 5] = [
        parse_quote!(1.2),
        parse_quote!(b"\0abc"),
        parse_quote!(false),
        parse_quote!('1'),
        parse_quote!(b'8'),
    ];
    for lit in &invalid_literals {
        assert_eq!(literal_to_tokens(lit), None);
    }
}

pub fn literal_to_path(lit: &Lit) -> Option<Path> {
    match *lit {
        Lit::Str(ref s) => syn::parse_str::<Path>(&s.value()).ok(),
        _ => None,
    }
}

#[test]
fn test_literal_to_path() {
    let valid_literals: [(Lit, Path); 4] = [
        (parse_quote!("Transaction"), parse_quote!(Transaction)),
        (
            parse_quote!("super::Transaction"),
            parse_quote!(super::Transaction),
        ),
        (
            parse_quote!("::some::path::Transaction"),
            parse_quote!(::some::path::Transaction),
        ),
        (
            parse_quote!("Transaction<'a>"),
            parse_quote!(Transaction<'a>),
        ),
    ];
    for &(ref lit, ref path) in &valid_literals {
        assert_eq!(literal_to_path(lit).unwrap(), *path);
    }

    let invalid_literals: [Lit; 4] =
        [parse_quote!("foo bar"), parse_quote!("1 + 2"), parse_quote!(false), parse_quote!('1')];
    for lit in &invalid_literals {
        assert_eq!(literal_to_path(lit), None);
    }
}

#[derive(Debug)]
pub enum AttrValue<T> {
    None,
    Some(T),
    Multiple,
}

/// Gets a list of key-value pairs inside #[exonum(..)] attributes.
pub fn named_attr(attrs: &[Attribute], name: &Ident) -> AttrValue<Lit> {
    let mut literals = attrs
        .iter()
        .filter(|attr| is_exonum_attr(attr))
        .filter_map(Attribute::interpret_meta)
        .filter_map(|meta| match meta {
            Meta::List(MetaList { nested, .. }) => Some(nested),
            _ => None,
        })
        .flat_map(|list| list.into_iter())
        .filter_map(|meta| match meta {
            NestedMeta::Meta(Meta::NameValue(MetaNameValue { ref ident, ref lit, .. }))
                if ident == name => Some(lit.clone()),
            _ => None,
        });

    match (literals.next(), literals.next()) {
        (Some(lit), None) => AttrValue::Some(lit),
        (None, ..) => AttrValue::None,
        _ => AttrValue::Multiple,
    }
}

pub fn execute_shifts<F>(variant: &VariantInfo, mut f: F) -> Tokens
where
    F: FnMut(usize, &BindingInfo) -> Tokens,
{
    let mut first = true;

    #[cfg_attr(rustfmt, rustfmt_skip)]
    variant
        .bindings()
        .iter()
        .enumerate()
        .fold(quote!(), |acc, (i, binding)| {
            let from_initializer = if first {
                first = false;
                quote!(0 as ::exonum::encoding::Offset)
            } else {
                quote!(__from + __size)
            };

            let field_type = strip_lifetimes(&binding.ast().ty);
            let emitted = f(i, binding);

            quote!(
                #acc
                let __from = #from_initializer;
                let __size = <#field_type as ::exonum::encoding::Field>::field_size();
                #emitted
            )
        })
}


#[test]
fn test_execute_shifts() {
    use synstructure::Structure;

    let input = parse_quote!(
        enum Transaction<'a> {
            Create { public_key: &'a PublicKey, name: &'a str },
            Transfer {
                #[exonum(author)]
                from: &'a PublicKey,
                to: &'a PublicKey,
                amount: u64,
            }
        }
    );
    let s = Structure::new(&input);

    let tokens = execute_shifts(&s.variants()[1], |_, binding| {
        let field_name = &binding.binding;
        quote!(writer.write(#field_name, __from, __from + __size);)
    });

    assert_eq!(tokens, quote!(
        let __from = 0 as ::exonum::encoding::Offset;
        let __size = <&PublicKey as ::exonum::encoding::Field>::field_size();
        writer.write(__binding_0, __from, __from + __size);
        let __from = __from + __size;
        let __size = <&PublicKey as ::exonum::encoding::Field>::field_size();
        writer.write(__binding_1, __from, __from + __size);
        let __from = __from + __size;
        let __size = <u64 as ::exonum::encoding::Field>::field_size();
        writer.write(__binding_2, __from, __from + __size);
    ));
}
