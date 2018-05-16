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

//! `RESTful` API and corresponding utilities. This module does not describe
//! the API itself, but rather the entities which the service author needs to
//! add private and public API endpoints to the service.
//!
//! The REST architectural style encompasses a list of constrains and properties
//! based on HTTP. The most common operations available are GET, POST, PUT and
//! DELETE. The requests are placed to a URL which represents a resource. The
//! requests either retrieve information or define a change that is to be made.

pub mod public;
pub mod private;

use iron::{status, IronError, headers::Cookie};
use iron::prelude::*;
use hyper::header::{ContentType, SetCookie};
use cookie::Cookie as CookiePair;
use router::Router;
use params;
use serde_json;
use serde::{Serialize, Serializer};
use serde::de::{self, Deserialize, Deserializer, Visitor};
use failure::Fail;
use bodyparser;

use std::{fmt, io};
use std::ops::Deref;
use std::marker::PhantomData;
use std::collections::BTreeMap;
use std::str::FromStr;

use crypto::{PublicKey, SecretKey};
use encoding::serialize::{encode_hex, FromHex, FromHexError, ToHex};
use storage;

#[cfg(test)]
mod tests;

/// List of possible API errors, which can be returned when processing an API
/// request.
#[derive(Fail, Debug)]
pub enum ApiError {
    /// Storage error. This error is returned, for example, if the requested data
    /// do not exist in the database; or if the requested data exists in the
    /// database but additional permissions are required to access it.
    #[fail(display = "Storage error: {}", _0)]
    Storage(#[cause] storage::Error),

    /// Input/output error.
    #[fail(display = "IO error: {}", _0)]
    Io(#[cause] ::std::io::Error),

    /// Bad request. This error is returned when the submitted request contains an
    /// invalid parameter.
    #[fail(display = "Bad request: {}", _0)]
    BadRequest(String),

    /// Not found. This error is returned when the path in the URL of the request
    /// is incorrect.
    #[fail(display = "Not found: {}", _0)]
    NotFound(String),

    /// Internal error. This this type of error can be defined
    #[fail(display = "Internal server error: {}", _0)]
    InternalError(Box<::std::error::Error + Send + Sync>),

    /// Unauthorized error. This error is returned when a user is not authorized
    /// in the system and does not have permissions to perform the API request.
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
            ApiError::NotFound(..) => status::NotFound,

            ApiError::Storage(..) | ApiError::Io(..) | ApiError::InternalError(..) => {
                status::InternalServerError
            }
        };
        let body = {
            let mut map = BTreeMap::new();
            map.insert("debug", format!("{:?}", e));
            map.insert("description", e.to_string());
            serde_json::to_string_pretty(&map).unwrap()
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
    T: AsRef<[u8]> + FromHex<Error = FromHexError> + ToHex + Clone,
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
    /// Deserializes a URL fragment as `T`.
    fn url_fragment<T>(&self, request: &Request, name: &str) -> Result<T, ApiError>
    where
        T: FromStr,
        T::Err: fmt::Display,
    {
        let params = request.extensions.get::<Router>().unwrap();
        let fragment = params.find(name).ok_or_else(|| {
            ApiError::BadRequest(format!("Required parameter '{}' is missing", name))
        })?;
        let value = T::from_str(fragment)
            .map_err(|e| ApiError::BadRequest(format!("Invalid '{}' parameter: {}", name, e)))?;
        Ok(value)
    }

    /// Deserializes an optional parameter from a request body or `GET` parameters.
    fn optional_param<T>(&self, request: &mut Request, name: &str) -> Result<Option<T>, ApiError>
    where
        T: FromStr,
        T::Err: fmt::Display,
    {
        let map = request.get_ref::<params::Params>().unwrap();
        let value = match map.find(&[name]) {
            Some(&params::Value::String(ref param)) => {
                let value = T::from_str(param).map_err(|e| {
                    ApiError::BadRequest(format!("Invalid '{}' parameter: {}", name, e))
                })?;
                Some(value)
            }
            _ => None,
        };
        Ok(value)
    }

    /// Deserializes a required parameter from a request body or `GET` parameters.
    fn required_param<T>(&self, request: &mut Request, name: &str) -> Result<T, ApiError>
    where
        T: FromStr,
        T::Err: fmt::Display,
    {
        self.optional_param(request, name)?.ok_or_else(|| {
            ApiError::BadRequest(format!("Required parameter '{}' is missing", name))
        })
    }

    /// Deserializes a request body as a structure of type `T`.
    fn parse_body<T: 'static>(&self, req: &mut Request) -> Result<T, ApiError>
    where
        T: Clone + for<'de> Deserialize<'de>,
    {
        match req.get::<bodyparser::Struct<T>>() {
            Ok(Some(param)) => Ok(param),
            Ok(None) => Err(ApiError::BadRequest("Body is empty".into())),
            Err(e) => Err(ApiError::BadRequest(format!("Invalid struct: {}", e))),
        }
    }

    /// Loads a hex value from the cookies.
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
        Err(storage::Error::new(format!(
            "Unable to find value with given key {}",
            key
        )))
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
    /// Returns NotFound and a certain with cookies.
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

    /// Returns OK and a certain response with cookies.
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

    /// Returns OK and a certain response.
    fn ok_response(&self, json: &serde_json::Value) -> IronResult<Response> {
        self.ok_response_with_cookies(json, None)
    }
    /// Returns NotFound and a certain response.
    fn not_found_response(&self, json: &serde_json::Value) -> IronResult<Response> {
        self.not_found_response_with_cookies(json, None)
    }

    /// Defines the URL through which certain internal methods can be applied.
    fn wire<'b>(&self, router: &'b mut Router);
}
