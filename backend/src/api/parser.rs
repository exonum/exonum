use iron::prelude::*;
use router::Router;
use params::{Value, Params};

use exonum::api::ApiError;
use exonum::crypto::Hash;
use exonum::encoding::serialize::FromHex;

pub trait TryParse: Sized {
    fn parse(s: &str) -> Result<Self, ApiError>;
}

impl TryParse for String {
    fn parse(s: &str) -> Result<Self, ApiError> {
        Ok(s.to_owned())
    }
}

impl TryParse for Hash {
    fn parse(s: &str) -> Result<Self, ApiError> {
        Hash::from_hex(s).map_err(|e| {
            let msg = format!("Unable to parse `{}`, an error occured: {}", s, e);
            ApiError::IncorrectRequest(msg.into())
        })
    }
}

impl TryParse for u64 {
    fn parse(s: &str) -> Result<Self, ApiError> {
        s.parse::<Self>().map_err(|e| {
            let msg = format!("Unable to parse `{}`, an error occured: {}", s, e);
            ApiError::IncorrectRequest(msg.into())
        })
    }
}

pub struct RequestParser<'a, 'b: 'a, 'c: 'b> {
    inner: &'a mut Request<'b, 'c>,
}

impl<'a, 'b: 'a, 'c: 'b> RequestParser<'a, 'b, 'c> {
    pub fn new(request: &'a mut Request<'b, 'c>) -> RequestParser<'a, 'b, 'c> {
        RequestParser { inner: request }
    }

    pub fn route_param<T: TryParse>(&self, name: &str) -> Result<T, ApiError> {
        let route = self.inner.extensions.get::<Router>().unwrap();
        let value = route.find(name).ok_or_else(|| {
            let msg = format!("Required parameter `{}` is missing", name);
            ApiError::IncorrectRequest(msg.into())
        })?;
        T::parse(value)
    }

    pub fn optional_param<T: TryParse>(&mut self, name: &str) -> Result<Option<T>, ApiError> {
        let params = self.inner.get_ref::<Params>().unwrap();
        match params.find(&[name]) {
            Some(&Value::String(ref s)) => Ok(Some(T::parse(s)?)),
            Some(_) => Err(ApiError::IncorrectRequest(
                "Unsupported type of param".into(),
            )),
            None => Ok(None),
        }
    }
}
