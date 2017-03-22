extern crate iron_test;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::path::Path;
use std::fs::File;
use std::io::Read;

use rand::{thread_rng, Rng};
use router::Router;
use iron::Headers;
use iron::status::Status;
use iron::prelude::*;
use iron::headers::ContentType;
use serde::Serialize;
use serde_json;
use serde_json::value::ToJson;


use exonum::storage::StorageValue; 
use exonum::node::{NodeConfig, TransactionSend};
use exonum::crypto::{Seed, Hash, PublicKey, gen_keypair, gen_keypair_from_seed};
use exonum::blockchain::{StoredConfiguration, Service, Transaction};
use exonum::events::Error as EventsError;
use exonum::messages::{FromRaw, Message, RawMessage};
use configuration_service::{ConfigTx, ConfigurationService, ConfigurationSchema};
use configuration_service::config_api::{ConfigWithHash, ConfigApi};

use blockchain_explorer::api::Api;
use blockchain_explorer::helpers::init_logger;

use sandbox::sandbox::{sandbox_with_services, Sandbox};
use sandbox::sandbox_tests_helper::{add_one_height_with_transactions, SandboxState};

fn response_body(response: Response) -> serde_json::Value {
    if let Some(mut body) = response.body {
        let mut buf = Vec::new();
        body.write_body(&mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        debug!("Received response body:'{}'", &s);
        serde_json::from_str(&s).unwrap()
    } else {
        serde_json::Value::Null
    }
}

fn request_get<A: AsRef<str>>(route: A, router: &Router) -> IronResult<Response> {
    info!("GET request:'{}'",
          format!("http://127.0.0.1:8000/{}", route.as_ref()));
    iron_test::request::get(&format!("http://127.0.0.1:8000/{}", route.as_ref()),
                            Headers::new(),
                            router)
}

fn request_post_str<B: AsRef<str>, A: AsRef<str>>(route: A,
                                                  body: B,
                                                  router: &Router)
                                                  -> IronResult<Response> {
    let body_str = body.as_ref();
    let mut headers = Headers::new();
    headers.set(ContentType::json());
    info!("POST request:'{}' with body '{}'",
          format!("http://127.0.0.1:8000/{}", route.as_ref()),
          body_str);
    iron_test::request::post(&format!("http://127.0.0.1:8000/{}", route.as_ref()),
                             headers,
                             body_str,
                             router)
}

fn request_post_body<T: Serialize, A: AsRef<str>>(route: A,
                                                  body: T,
                                                  router: &Router)
                                                  -> IronResult<Response> {
    let body_str: &str = &serde_json::to_string(&body).unwrap();
    request_post_str(route, body_str, router)
}

fn from_file<P: AsRef<Path>>(path: P) -> serde_json::Value {
    let mut file = File::open(path).unwrap();
    let mut s = String::new();
    file.read_to_string(&mut s).unwrap();
    serde_json::from_str(&s).unwrap()
}

#[derive(Clone)]
struct TestTxSender {
    transactions: Arc<Mutex<VecDeque<RawMessage>>>,
}

impl TransactionSend for TestTxSender {
    fn send<T: Transaction>(&self, tx: T) -> Result<(), EventsError> {
        if !tx.verify() {
            return Err(EventsError::new("Unable to verify transaction"));
        }
        let rm = tx.raw().clone();
        self.transactions.lock().unwrap().push_back(rm);
        Ok(())
    }
}

struct ConfigurationApiSandbox {
    sandbox: Sandbox,
    state: SandboxState,
    transactions: Arc<Mutex<VecDeque<RawMessage>>>,
}


impl ConfigurationApiSandbox {
    fn new() -> ConfigurationApiSandbox {
        let services: Vec<Box<Service>> = vec![Box::new(ConfigurationService::new())];
        let sandbox = sandbox_with_services(services);
        let state = SandboxState::new();
        ConfigurationApiSandbox {
            sandbox: sandbox,
            state: state,
            transactions: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    fn obtain_test_api(&self, validator_id: usize) -> Router {
        let channel = TestTxSender { transactions: self.transactions.clone() };
        let blockchain = self.sandbox.blockchain_ref().clone();
        let keypair = (self.sandbox.p(validator_id), self.sandbox.s(validator_id).clone());
        let api = ConfigApi {
            channel: channel,
            blockchain: blockchain,
            config: keypair, 
        };
        let mut router = Router::new();
        api.wire(&mut router);
        router
    }

    fn commit(&self) {
        let mut collected_transactions = self.transactions.lock().unwrap();
        let txs = collected_transactions.drain(..).collect::<Vec<_>>();
        debug!("Sandbox commits a sequence of {} transactions", txs.len());
        txs.iter()
            .inspect(|elem| {
                trace!("Message hash: {:?}", Message::hash(*elem));
                trace!("{:?}", ConfigTx::from_raw((*elem).clone()));
            })
            .collect::<Vec<_>>();
        add_one_height_with_transactions(&self.sandbox, &self.state, txs.iter());
    }

    fn request_actual_config(&self)
                                              -> IronResult<Response> {
        let api = self.obtain_test_api(0);
        request_get("/api/v1/config/actual", &api)
    }

    fn request_following_config(&self)
                                              -> IronResult<Response> {
        let api = self.obtain_test_api(0);
        request_get("/api/v1/config/following", &api)
    }
}

#[derive(Deserialize)]
struct TxResponse {
    tx_hash: Hash,
}

#[derive(Deserialize)]
struct ErrorResponse {
    #[serde(rename = "type")]
    type_str: String,
}

#[test]
fn test_get_actual_config() {
    let _ = init_logger();
    let api_sandbox = ConfigurationApiSandbox::new();
    let resp_actual_config = api_sandbox.request_actual_config().unwrap(); 
    let actual_body = response_body(resp_actual_config);
    let sand_cfg = api_sandbox.sandbox.cfg();
    let expected_response_body = ConfigWithHash{
        hash: sand_cfg.hash(), 
        config: sand_cfg, 
    };
    assert_eq!(actual_body, expected_response_body.to_json());

    let resp_following_config = api_sandbox.request_following_config().unwrap(); 
    let actual_body = response_body(resp_following_config);
    let expected_body: Option<ConfigWithHash> = None;
    assert_eq!(actual_body, expected_body.to_json());
}

fn assert_response_status(response: IronResult<Response>,
                          expected_status: Status,
                          expected_message: &str) {
    assert!(response.is_err());
    match response {
        Err(iron_error) => {
            let resp = iron_error.response;
            debug!("Error response: {}", resp);
            assert_eq!(resp.status, Some(expected_status));
            let body = response_body(resp);
            let error_body = serde_json::from_value::<ErrorResponse>(body).unwrap();
            assert_eq!(&error_body.type_str, expected_message);
        } 
        _ => unreachable!(), 
    }
}



