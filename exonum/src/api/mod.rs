// Copyright 2017 The Exonum Team
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

//! `RESTful` API and corresponding utilities.

use std::ops::Deref;
use std::marker::PhantomData;
use std::io;
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use iron::IronError;
use iron::prelude::*;
use iron::status;
use iron::headers::Cookie;
use hyper::header::{ContentType, SetCookie};
use cookie::Cookie as CookiePair;
use router::Router;
use params;
use serde_json;
use serde::{Serialize, Serializer};
use serde::de::{self, Visitor, Deserialize, Deserializer};
use failure::Fail;

use crypto::{PublicKey, SecretKey};
use encoding::serialize::{FromHex, FromHexError, ToHex, encode_hex};
use storage;

pub mod public;
pub mod private;
#[cfg(test)]
mod tests;

/// List of possible Api errors.
#[derive(Fail, Debug)]
pub enum ApiError {
    /// Storage error.
    #[fail(display = "Storage error: {}", _0)]
    Storage(
        #[cause]
        storage::Error
    ),

    /// Input/output error.
    #[fail(display = "IO error: {}", _0)]
    Io(
        #[cause]
        ::std::io::Error
    ),

    /// Bad request.
    #[fail(display = "Bad request: {}", _0)]
    BadRequest(String),

    /// Internal error.
    #[fail(display = "Internal server error: {}", _0)]
    InternalError(Box<::std::error::Error + Send + Sync>),

    /// Unauthorized error.
    #[fail(display = "Unauthorized")]
    Unauthorized,
}

impl From<io::Error> for ApiError {
    fn from(e: io::Error) -> ApiError {
        ApiError::Io(e)
    }
}

impl From<storage::Error> for ApiError {
    fn from(e: storage::Error) -> ApiError {
        ApiError::Storage(e)
    }
}

impl From<ApiError> for IronError {
    fn from(e: ApiError) -> IronError {
        let code = match e {
            // Note that `status::Unauthorized` does not fit here, because
            //
            // > A server generating a 401 (Unauthorized) response MUST send a
            // > WWW-Authenticate header field containing at least one challenge.
            //
            // https://tools.ietf.org/html/rfc7235#section-4.1
            ApiError::Unauthorized => status::Forbidden,

            ApiError::BadRequest(..) => status::BadRequest,

            ApiError::Storage(..) |
            ApiError::Io(..) |
            ApiError::InternalError(..) => status::InternalServerError,
        };
        let body = {
            let mut map = BTreeMap::new();
            map.insert("debug", format!("{:?}", e));
            map.insert("description", e.to_string());
            ::serde_json::to_string_pretty(&map).unwrap()
        };
        IronError::new(e.compat(), (code, body))
    }
}

/// `Field` that is serialized/deserialized from/to hex.
#[derive(Clone, Debug)]
struct HexField<T: AsRef<[u8]> + Clone>(pub T);

impl<T> Deref for HexField<T>
where
    T: AsRef<[u8]> + Clone,
{
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> Serialize for HexField<T>
where
    T: AsRef<[u8]> + Clone,
{
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ser.serialize_str(&encode_hex(&self.0))
    }
}

struct HexVisitor<T>
where
    T: AsRef<[u8]> + Clone + FromHex<Error = FromHexError>,
{
    _p: PhantomData<T>,
}

impl<'v, T> Visitor<'v> for HexVisitor<T>
where
    T: AsRef<[u8]> + Clone + FromHex<Error = FromHexError>,
{
    type Value = HexField<T>;

    fn expecting(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(fmt, "expected hex represented string")
    }

    fn visit_str<E>(self, s: &str) -> Result<HexField<T>, E>
    where
        E: de::Error,
    {
        let v = T::from_hex(s).map_err(|_| de::Error::custom("Invalid hex"))?;
        Ok(HexField(v))
    }
}

impl<'de, T> Deserialize<'de> for HexField<T>
where
    T: AsRef<[u8]>
        + FromHex<Error = FromHexError>
        + ToHex
        + Clone,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(HexVisitor { _p: PhantomData })
    }
}

/// `Api` trait defines `RESTful` API.
pub trait Api {
    /// Loads hex value from the cookies.
    fn load_hex_value_from_cookie<'a>(
        &self,
        request: &'a Request,
        key: &str,
    ) -> storage::Result<Vec<u8>> {
        if let Some(&Cookie(ref cookies)) = request.headers.get() {
            for cookie in cookies.iter() {
                if let Ok(c) = CookiePair::parse(cookie.as_str()) {
                    if c.name() == key {
                        if let Ok(value) = FromHex::from_hex(c.value()) {
                            return Ok(value);
                        }
                    }
                }
            }
        }
        Err(storage::Error::new(
            format!("Unable to find value with given key {}", key),
        ))
    }

    /// Loads public and secret key from the cookies.
    fn load_keypair_from_cookies(
        &self,
        request: &Request,
    ) -> Result<(PublicKey, SecretKey), ApiError> {
        let public_key = PublicKey::from_slice(
            self.load_hex_value_from_cookie(request, "public_key")?
                .as_ref(),
        );
        let secret_key = SecretKey::from_slice(
            self.load_hex_value_from_cookie(request, "secret_key")?
                .as_ref(),
        );

        let public_key = public_key.ok_or(ApiError::Unauthorized)?;
        let secret_key = secret_key.ok_or(ApiError::Unauthorized)?;
        Ok((public_key, secret_key))
    }

    //TODO: Remove duplicate code
    /// Returns NotFound and some response with cookies.
    fn not_found_response_with_cookies(
        &self,
        json: &serde_json::Value,
        cookies: Option<Vec<String>>,
    ) -> IronResult<Response> {
        let mut resp = Response::with((
            status::NotFound,
            serde_json::to_string_pretty(json).unwrap(),
        ));
        resp.headers.set(ContentType::json());
        if let Some(cookies) = cookies {
            resp.headers.set(SetCookie(cookies));
        }
        Ok(resp)
    }

    /// Returns OK and some response with cookies.
    fn ok_response_with_cookies(
        &self,
        json: &serde_json::Value,
        cookies: Option<Vec<String>>,
    ) -> IronResult<Response> {
        let mut resp = Response::with((status::Ok, serde_json::to_string_pretty(json).unwrap()));
        resp.headers.set(ContentType::json());
        if let Some(cookies) = cookies {
            resp.headers.set(SetCookie(cookies));
        }
        Ok(resp)
    }

    /// Returns OK and some response.
    fn ok_response(&self, json: &serde_json::Value) -> IronResult<Response> {
        self.ok_response_with_cookies(json, None)
    }
    /// Returns NotFound and some response.
    fn not_found_response(&self, json: &serde_json::Value) -> IronResult<Response> {
        self.not_found_response_with_cookies(json, None)
    }

    /// Used to extend Api.
    fn wire<'b>(&self, router: &'b mut Router);
}

fn url_fragment<T>(request: &Request, name: &str) -> Result<T, ApiError>
where
    T: FromStr,
    T::Err: fmt::Display,
{
    let params = request.extensions.get::<Router>().unwrap();
    let fragment = params.find(name).ok_or_else(|| {
        ApiError::BadRequest(format!("Required parameter '{}' is missing", name))
    })?;
    let value: T = T::from_str(fragment).map_err(|e| {
        ApiError::BadRequest(format!("Invalid '{}' parameter: {}", name, e))
    })?;
    Ok(value)
}

fn optional_param<T>(request: &mut Request, name: &str) -> Result<Option<T>, ApiError>
where
    T: FromStr,
    T::Err: fmt::Display,
{
    let map = request.get_ref::<params::Params>().unwrap();
    let value = match map.find(&[name]) {
        Some(&params::Value::String(ref param)) => {
            let value: T = T::from_str(param).map_err(|e| {
                ApiError::BadRequest(format!("Invalid '{}' parameter: {}", name, e))
            })?;
            Some(value)
        }
        _ => None,
    };
    Ok(value)
}

fn required_param<T>(request: &mut Request, name: &str) -> Result<T, ApiError>
where
    T: FromStr,
    T::Err: fmt::Display,
{
    optional_param(request, name)?.ok_or_else(|| {
        ApiError::BadRequest(format!("Required parameter '{}' is missing", name))
    })
}
