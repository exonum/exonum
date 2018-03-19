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

//! `Payload` wrapper.

use quote::Tokens;
use syn::{Attribute, Ident, Generics, GenericParam, Lifetime, LifetimeDef, Path};
use synstructure::Structure;

use utils::{literal_to_path, literal_to_tokens, named_attr, AttrValue};

#[derive(Debug, Fail, PartialEq)]
pub enum ParseError {
    #[fail(display = "missing `#[exonum(service_id)]` attribute")]
    NoServiceId,
    #[fail(display = "malformed `#[exonum(service_id)]` attribute (should be \
        a `u16` value or expression)")]
    MalformedServiceId,
    #[fail(display = "duplicate `#[exonum(service_id)]` attribute")]
    DuplicateServiceId,

    #[fail(display = "unsupported generic params in messages declaration; \
        at most one lifetime param is supported")]
    UnsupportedGenerics,

    #[cfg_attr(rustfmt, rustfmt_skip)]
    #[fail(display = "invalid type for `Message` derivation. Use a newtype \
        wrapping `RawMessage`: `struct {}(RawMessage)`", _0)]
    InvalidMessageStruct(String),

    #[fail(display = "`#[exonum(payload = \"path::to::Payload\")]` attribute not specified")]
    NoPayload,
    #[fail(display = "malformed `#[exonum(payload)]` attribute (should have the form \
        `#[exonum(payload = \"path::to::Payload\")]`)")]
    MalformedPayload,
    #[fail(display = "`#[exonum(payload)]` attribute specified multiple times")]
    DuplicatePayload,
}

pub trait UnwrapOrEmit<T> {
    fn unwrap_or_emit(self) -> T;
}

impl<T> UnwrapOrEmit<T> for Result<T, ParseError> {
    fn unwrap_or_emit(self) -> T {
        match self {
            Ok(value) => value,
            Err(error) => panic!("{}", error),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct MessageInfo {
    /// `message_id`
    id: Option<Tokens>,
    /// Index of the field marked with `author` attr.
    author: Option<usize>,
}

pub struct Payload<'a> {
    pub ids_enum_ident: Ident,
    pub lifetime: Option<Lifetime>,
    pub service_id: Tokens,
    structure: Structure<'a>,
}

impl<'a> Payload<'a> {
    pub fn try_from(structure: Structure<'a>) -> Result<Self, ParseError> {
        let ids_enum_ident = Ident::from(format!("{}Ids", structure.ast().ident));
        let lifetime = extract_lifetime(&structure.ast().generics)?;
        let service_id = extract_service_id(&structure.ast().attrs)?;

        Ok(Payload {
            ids_enum_ident,
            structure,
            lifetime,
            service_id,
        })
    }

    pub fn id_pat(&self, i: usize) -> Tokens {
        let ids_enum = &self.ids_enum_ident;
        let ident = &self.structure.variants()[i].ast().ident;
        quote!(x if x == #ids_enum::#ident as u16)
    }
}

impl<'a> ::std::ops::Deref for Payload<'a> {
    type Target = Structure<'a>;

    fn deref(&self) -> &Structure<'a> {
        &self.structure
    }
}

fn extract_lifetime(generics: &Generics) -> Result<Option<Lifetime>, ParseError> {
    if generics.params.len() > 1 {
        return Err(ParseError::UnsupportedGenerics);
    }

    let param = generics.params.first().map(|param| *param.value());
    match param {
        Some(&GenericParam::Lifetime(LifetimeDef { ref lifetime, .. })) => Ok(Some(*lifetime)),
        None => Ok(None),
        _ => Err(ParseError::UnsupportedGenerics),
    }
}

#[test]
fn test_extract_lifetime() {
    use quote::ToTokens;

    let input = parse_quote!(
        #[exonum(service_id = 5)]
        enum Transaction<'foo> { }
    );
    let s = Structure::new(&input);

    assert_eq!(
        extract_lifetime(&s.ast().generics).map(|opt| opt.map(ToTokens::into_tokens)),
        Ok(Some(quote!('foo)))
    );

    let input = parse_quote!(
        #[exonum(service_id = 5)]
        enum Transaction { }
    );
    let s = Structure::new(&input);

    assert_eq!(
        extract_lifetime(&s.ast().generics).map(|opt| opt.map(ToTokens::into_tokens)),
        Ok(None)
    );

    let input = parse_quote!(
        #[exonum(service_id = 5)]
        enum Transaction<'a, 'b> { }
    );
    let s = Structure::new(&input);

    assert_eq!(
        extract_lifetime(&s.ast().generics).map(|opt| opt.map(ToTokens::into_tokens)),
        Err(ParseError::UnsupportedGenerics)
    );

    let input = parse_quote!(
        #[exonum(service_id = 5)]
        enum Transaction<'a, T> {
            Foo { bar: &'a T }
        }
    );
    let s = Structure::new(&input);

    assert_eq!(
        extract_lifetime(&s.ast().generics).map(|opt| opt.map(ToTokens::into_tokens)),
        Err(ParseError::UnsupportedGenerics)
    );
}

fn extract_service_id(attrs: &[Attribute]) -> Result<Tokens, ParseError> {
    match named_attr(attrs, &Ident::from("service_id")) {
        AttrValue::Some(id) => literal_to_tokens(&id).ok_or(ParseError::MalformedServiceId),
        AttrValue::None => Err(ParseError::NoServiceId),
        _ => Err(ParseError::DuplicateServiceId),
    }
}

#[test]
fn test_extract_service_id() {
    let input = parse_quote!(
        #[exonum(service_id = 5)]
        enum Transaction<'a> { }
    );
    let s = Structure::new(&input);

    assert_eq!(extract_service_id(&s.ast().attrs), Ok(quote!(5u16)));

    let input = parse_quote!(
        #[exonum(service_id = "SERVICE_ID", some_other_attr)]
        enum Transaction<'a> { }
    );
    let s = Structure::new(&input);

    assert_eq!(extract_service_id(&s.ast().attrs), Ok(quote!(SERVICE_ID)));

    let input = parse_quote!(
        enum Transaction<'a> { }
    );
    let s = Structure::new(&input);

    assert_eq!(
        extract_service_id(&s.ast().attrs),
        Err(ParseError::NoServiceId)
    );

    let input = parse_quote!(
        #[exonum(service_i = 1)]
        enum Transaction<'a> { }
    );
    let s = Structure::new(&input);

    assert_eq!(
        extract_service_id(&s.ast().attrs),
        Err(ParseError::NoServiceId)
    );

    let input = parse_quote!(
        #[exonum(service_id = 1, service_id = "FOO")]
        enum Transaction<'a> { }
    );
    let s = Structure::new(&input);

    assert_eq!(
        extract_service_id(&s.ast().attrs),
        Err(ParseError::DuplicateServiceId)
    );

    let input = parse_quote!(
        #[exonum(service_id = 1)]
        #[exonum(service_id = "FOO")]
        enum Transaction<'a> { }
    );
    let s = Structure::new(&input);

    assert_eq!(
        extract_service_id(&s.ast().attrs),
        Err(ParseError::DuplicateServiceId)
    );

    let input = parse_quote!(
        #[exonum(service_id = false)]
        enum Transaction<'a> { }
    );
    let s = Structure::new(&input);

    assert_eq!(
        extract_service_id(&s.ast().attrs),
        Err(ParseError::MalformedServiceId)
    );
}

pub fn check_message_struct(s: &Structure) -> Result<(), ParseError> {
    use syn::{Data, DataStruct, Fields, FieldsUnnamed};

    match s.ast().data {
        Data::Struct(DataStruct {
                         fields: Fields::Unnamed(FieldsUnnamed { ref unnamed, .. }), ..
                     }) if unnamed.len() == 1 => Ok(()),
        _ => {
            return Err(ParseError::InvalidMessageStruct(s.ast().ident.to_string()));
        }
    }
}

pub fn extract_payload(attrs: &[Attribute]) -> Result<Path, ParseError> {
    match named_attr(attrs, &Ident::from("payload")) {
        AttrValue::Some(id) => literal_to_path(&id).ok_or(ParseError::MalformedPayload),
        AttrValue::None => Err(ParseError::NoPayload),
        _ => Err(ParseError::DuplicatePayload),
    }
}
