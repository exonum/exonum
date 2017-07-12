use iron::IronError;
use iron::prelude::*;
use iron::status;
use iron::headers::Cookie;
use hyper::header::{ContentType, SetCookie};
use cookie::Cookie as CookiePair;
use router::Router;
use serde_json;
use serde::{Serialize, Serializer};
use serde::de::{self, Visitor, Deserialize, Deserializer};

use std::ops::Deref;
use std::marker::PhantomData;
use std::io;
use std::collections::BTreeMap;
use std::fmt;

use events::Error as EventsError;
use crypto::{PublicKey, SecretKey, HexValue, FromHexError, Hash};
use encoding::serialize::ToHex;
use storage::{Result as StorageResult, Error as StorageError};

#[derive(Debug)]
pub enum ApiError {
    Service(Box<::std::error::Error + Send + Sync>),
    Storage(StorageError),
    Events(EventsError),
    FromHex(FromHexError),
    Io(::std::io::Error),
    FileNotFound(Hash),
    NotFound,
    FileTooBig,
    FileExists(Hash),
    IncorrectRequest(Box<::std::error::Error + Send + Sync>),
    Unauthorized,
    AddressParseError(::std::net::AddrParseError),
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl ::std::error::Error for ApiError {
    fn description(&self) -> &str {
        match *self {
            ApiError::Service(ref error) |
            ApiError::IncorrectRequest(ref error) => error.description(),
            ApiError::Storage(ref error) => error.description(),
            ApiError::Events(ref error) => error.description(),
            ApiError::FromHex(ref error) => error.description(),
            ApiError::Io(ref error) => error.description(),
            ApiError::FileNotFound(_) => "File not found",
            ApiError::NotFound => "Not found",
            ApiError::FileTooBig => "File too big",
            ApiError::FileExists(_) => "File exists",
            ApiError::Unauthorized => "Unauthorized",
            ApiError::AddressParseError(_) => "AddressParseError",
        }
    }
}

impl From<::std::net::AddrParseError> for ApiError {
    fn from(e: ::std::net::AddrParseError) -> ApiError {
        ApiError::AddressParseError(e)
    }
}

impl From<io::Error> for ApiError {
    fn from(e: io::Error) -> ApiError {
        ApiError::Io(e)
    }
}

impl From<StorageError> for ApiError {
    fn from(e: StorageError) -> ApiError {
        ApiError::Storage(e)
    }
}

impl From<EventsError> for ApiError {
    fn from(e: EventsError) -> ApiError {
        ApiError::Events(e)
    }
}

impl From<FromHexError> for ApiError {
    fn from(e: FromHexError) -> ApiError {
        ApiError::FromHex(e)
    }
}

impl From<ApiError> for IronError {
    fn from(e: ApiError) -> IronError {
        use std::error::Error;

        let mut body = BTreeMap::new();
        body.insert("debug", format!("{:?}", e));
        body.insert("description", e.description().to_string());
        let code = match e {
            ApiError::FileExists(hash) |
            ApiError::FileNotFound(hash) => {
                body.insert("hash", ToHex::to_hex(&hash));
                status::Conflict
            }
            _ => status::Conflict,
        };
        IronError {
            error: Box::new(e),
            response: Response::with((code, ::serde_json::to_string_pretty(&body).unwrap())),
        }
    }
}

#[derive(Clone, Debug)]
pub struct HexField<T: AsRef<[u8]> + Clone>(pub T);

impl<T> Deref for HexField<T>
    where T: AsRef<[u8]> + Clone
{
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> Serialize for HexField<T>
    where T: AsRef<[u8]> + Clone
{
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        ser.serialize_str(&self.0.as_ref().to_hex())
    }
}

struct HexVisitor<T>
    where T: AsRef<[u8]> + HexValue
{
    _p: PhantomData<T>,
}

impl<'v, T> Visitor<'v> for HexVisitor<T>
    where T: AsRef<[u8]> + HexValue + Clone
{
    type Value = HexField<T>;

    fn expecting(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(fmt, "expected hex represented string")
    }

    fn visit_str<E>(self, s: &str) -> Result<HexField<T>, E>
        where E: de::Error
    {
        let v = T::from_hex(s)
            .map_err(|_| de::Error::custom("Invalid hex"))?;
        Ok(HexField(v))
    }
}

impl<'de, T> Deserialize<'de> for HexField<T>
    where T: AsRef<[u8]> + HexValue + Clone
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>
    {
        deserializer.deserialize_str(HexVisitor { _p: PhantomData })
    }
}

pub trait Api {
    fn load_hex_value_from_cookie<'a>(&self,
                                      request: &'a Request,
                                      key: &str)
                                      -> StorageResult<Vec<u8>> {
        if let Some(&Cookie(ref cookies)) = request.headers.get() {
            for cookie in cookies.iter() {
                if let Ok(c) = CookiePair::parse(cookie.as_str()) {
                    if c.name() == key {
                        if let Ok(value) = HexValue::from_hex(c.value()) {
                            return Ok(value);
                        }
                    }
                }
            }
        }
        Err(StorageError::new(format!("Unable to find value with given key {}", key)))
    }

    fn load_keypair_from_cookies(&self,
                                 request: &Request)
                                 -> Result<(PublicKey, SecretKey), ApiError> {
        let public_key = PublicKey::from_slice(self.load_hex_value_from_cookie(request,
                                                                               "public_key")?
                                                   .as_ref());
        let secret_key = SecretKey::from_slice(self.load_hex_value_from_cookie(request,
                                                                               "secret_key")?
                                                   .as_ref());

        let public_key = public_key.ok_or(ApiError::Unauthorized)?;
        let secret_key = secret_key.ok_or(ApiError::Unauthorized)?;
        Ok((public_key, secret_key))
    }

    fn ok_response_with_cookies(&self,
                                json: &serde_json::Value,
                                cookies: Option<Vec<String>>)
                                -> IronResult<Response> {
        let mut resp = Response::with((status::Ok, serde_json::to_string_pretty(json).unwrap()));
        resp.headers.set(ContentType::json());
        if let Some(cookies) = cookies {
            resp.headers.set(SetCookie(cookies));
        }
        Ok(resp)
    }

    fn ok_response(&self, json: &serde_json::Value) -> IronResult<Response> {
        self.ok_response_with_cookies(json, None)
    }

    fn wire<'b>(&self, router: &'b mut Router);
}

#[cfg(test)]
mod tests {
    use router::Router;
    use serde_json;

    use blockchain::{Block, SCHEMA_MAJOR_VERSION};
    use crypto::Hash;

    use super::*;

    #[test]
    fn test_json_response_for_complex_val() {
        let str_val = "sghdkgskgskldghshgsd";
        let txs = [34, 32];
        let tx_count = txs.len() as u32;
        let complex_val = Block::new(SCHEMA_MAJOR_VERSION,
                                     0,
                                     24,
                                     tx_count,
                                     &Hash::new([24; 32]),
                                     &Hash::new([34; 32]),
                                     &Hash::new([38; 32]));
        struct SampleAPI;
        impl Api for SampleAPI {
            fn wire<'b>(&self, _: &'b mut Router) {
                return;
            }
        }
        let stub = SampleAPI;
        let result = stub.ok_response(&serde_json::to_value(str_val).unwrap());
        assert!(result.is_ok());
        let result = stub.ok_response(&serde_json::to_value(&complex_val).unwrap());
        assert!(result.is_ok());
        print!("{:?}", result);
    }
}
