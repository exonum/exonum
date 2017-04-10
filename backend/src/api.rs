use std;
use std::io;
use std::collections::BTreeMap;

use serde_json::value::ToJson;
use router::Router;
use blockchain_explorer::api::Api;
use iron::prelude::*;
use iron::status;
use iron::mime::{Mime, TopLevel, SubLevel};
use params::{Params, Value};
use params;
use time;

use exonum::crypto::{Hash, HexValue, FromHexError, Signature};
use exonum::blockchain::Blockchain;
use exonum::storage::{Map, Error as StorageError};
use exonum::events::Error as EventsError;
use exonum::node::TransactionSend;

use {TimestampTx, TimestampingSchema, Content};

#[derive(Clone)]
pub struct TimestampingApi<T: TransactionSend + Clone> {
    channel: T,
    blockchain: Blockchain,
}

#[derive(Debug)]
enum ApiError {
    Storage(StorageError),
    Events(EventsError),
    FromHex(FromHexError),
    Io(::std::io::Error),
    FileNotFound(Hash),
    FileToBig,
    FileExists(Hash),
    IncorrectRequest,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for ApiError {
    fn description(&self) -> &str {
        match *self {
            ApiError::Storage(_) => "Storage",
            ApiError::Events(_) => "Events",
            ApiError::FromHex(_) => "FromHex",
            ApiError::Io(_) => "Io",
            ApiError::FileNotFound(_) => "FileNotFound",
            ApiError::FileToBig => "FileToBig",
            ApiError::FileExists(_) => "FileExists",
            ApiError::IncorrectRequest => "IncorrectRequest",
        }
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
        body.insert("type", e.description().into());
        let code = match e {
            ApiError::FileExists(hash) => {
                body.insert("hash", hash.to_hex());
                status::Conflict
            }
            ApiError::FileNotFound(hash) => {
                body.insert("hash", hash.to_hex());
                status::Conflict
            }
            _ => status::Conflict,
        };
        IronError {
            error: Box::new(e),
            response: Response::with((code, body.to_json().to_string())),
        }
    }
}

impl<T> TimestampingApi<T>
    where T: TransactionSend + Clone
{
    pub fn new(blockchain: Blockchain, channel: T) -> TimestampingApi<T> {
        TimestampingApi {
            blockchain: blockchain,
            channel: channel,
        }
    }

    fn put_content(&self, hash_str: &str, description: &str) -> Result<TimestampTx, ApiError> {
        let hash = Hash::from_hex(hash_str)?;
        let view = self.blockchain.view();

        if TimestampingSchema::new(&view)
               .contents()
               .get(&hash)?
               .is_some() {
            return Err(ApiError::FileExists(hash));
        }
        // Create transaction
        let ts = time::now_utc().to_timespec();
        let tx = TimestampTx::new_with_signature(&description, ts, &hash, &Signature::zero());
        self.channel.send(tx.clone())?;
        Ok(tx)
    }

    fn get_content(&self, hash_str: &str) -> Result<Content, ApiError> {
        let hash = Hash::from_hex(hash_str)?;
        let view = self.blockchain.view();
        TimestampingSchema::new(&view)
            .contents()
            .get(&hash)?
            .ok_or_else(|| ApiError::FileNotFound(hash))
    }
}

impl<T> Api for TimestampingApi<T>
    where T: TransactionSend + Clone + 'static
{
    fn wire(&self, router: &mut Router) {
        // Receive a message by POST and play it back.
        let api = self.clone();
        let put_content = move |req: &mut Request| -> IronResult<Response> {
            let map = req.get_ref::<Params>().unwrap();

            fn find_str<'a>(map: &'a params::Map, path: &[&str]) -> Result<&'a str, ApiError> {
                let value = map.find(path);
                if let Some(&Value::String(ref s)) = value {
                    Ok(s)
                } else {
                    Err(ApiError::IncorrectRequest)
                }
            };

            let hash = find_str(map, &["hash"])?;
            let description = find_str(map, &["description"]).unwrap_or("");

            let tx = api.put_content(hash, description)?;
            let content_type = Mime(TopLevel::Application, SubLevel::Json, Vec::new());
            let response = Response::with((content_type, status::Ok, tx.to_json().to_string()));
            return Ok(response);
        };

        let api = self.clone();
        let get_content = move |req: &mut Request| -> IronResult<Response> {
            let ref hash = req.extensions
                .get::<Router>()
                .unwrap()
                .find("hash")
                .unwrap();
            let content = api.get_content(&hash)?;

            let content_type = Mime(TopLevel::Application, SubLevel::Json, Vec::new());
            let response =
                Response::with((content_type, status::Ok, content.to_json().to_string()));
            Ok(response)
        };

        router.get("/timestamping/content/:hash", get_content, "get_content");
        router.post("/timestamping/content", put_content, "put_content");
    }
}