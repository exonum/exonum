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

extern crate iron_test;

use rand::{thread_rng, Rng};
use router::Router;
use iron::Headers;
use iron::status::Status;
use iron::prelude::*;
use iron::headers::ContentType;
use serde::Serialize;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::path::Path;
use std::fs::File;
use std::io::Read;
use std::str;

use exonum::storage::{Database, MemoryDB, ProofListIndex, StorageValue};
use exonum::node::TransactionSend;
use exonum::crypto::Hash;
use exonum::blockchain::{Service, Transaction};
use exonum::events::Error as EventsError;
use exonum::messages::{Message, RawMessage};
use exonum::api::Api;
use exonum::helpers::{init_logger, ValidatorId, Height};
use exonum::encoding::serialize::json::reexport as serde_json;
use sandbox::sandbox::{sandbox_with_services, Sandbox};
use sandbox::sandbox_tests_helper::{add_one_height_with_transactions, SandboxState};

use exonum_configuration::{StorageValueConfigProposeData, TxConfigPropose, TxConfigVote,
                           ConfigurationService, ZEROVOTE};
use exonum_configuration::config_api::{PublicConfigApi, PrivateConfigApi, ApiResponseConfigInfo,
                                       ApiResponseConfigHashInfo, ApiResponseVotesInfo,
                                       ApiResponseProposePost, ApiResponseVotePost};
use super::generate_config_with_message;

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
    info!(
        "GET request:'{}'",
        format!("http://127.0.0.1:8000/{}", route.as_ref())
    );
    iron_test::request::get(
        &format!("http://127.0.0.1:8000/{}", route.as_ref()),
        Headers::new(),
        router,
    )
}

fn request_post_str<B: AsRef<str>, A: AsRef<str>>(
    route: A,
    body: B,
    router: &Router,
) -> IronResult<Response> {
    let body_str = body.as_ref();
    let mut headers = Headers::new();
    headers.set(ContentType::json());
    info!(
        "POST request:'{}' with body '{}'",
        format!("http://127.0.0.1:8000/{}", route.as_ref()),
        body_str
    );
    iron_test::request::post(
        &format!("http://127.0.0.1:8000/{}", route.as_ref()),
        headers,
        body_str,
        router,
    )
}

fn request_post_body<T: Serialize, A: AsRef<str>>(
    route: A,
    body: T,
    router: &Router,
) -> IronResult<Response> {
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
    fn send(&self, tx: Box<Transaction>) -> Result<(), EventsError> {
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

    fn obtain_test_api(&self, validator_id: ValidatorId) -> Router {
        let channel = TestTxSender { transactions: self.transactions.clone() };
        let blockchain = self.sandbox.blockchain_ref().clone();
        let keypair = (
            self.sandbox.service_public_key(validator_id),
            self.sandbox.service_secret_key(validator_id).clone(),
        );
        let pub_api = PublicConfigApi { blockchain: blockchain.clone() };
        let priv_api = PrivateConfigApi {
            channel: channel,
            config: keypair,
        };
        let mut router = Router::new();
        pub_api.wire(&mut router);
        priv_api.wire(&mut router);
        router
    }

    fn commit(&self) {
        let mut collected_transactions = self.transactions.lock().unwrap();
        let txs = collected_transactions.drain(..).collect::<Vec<_>>();
        debug!("Sandbox commits a sequence of {} transactions", txs.len());
        txs.iter()
            .inspect(|elem| {
                trace!("Message hash: {:?}", Message::hash(*elem));
                trace!("{:?}", (*elem).clone());
            })
            .collect::<Vec<_>>();
        add_one_height_with_transactions(&self.sandbox, &self.state, txs.iter());
    }

    fn get_actual_config(&self) -> IronResult<Response> {
        let api = self.obtain_test_api(ValidatorId::zero());
        request_get("/v1/configs/actual", &api)
    }

    fn get_following_config(&self) -> IronResult<Response> {
        let api = self.obtain_test_api(ValidatorId::zero());
        request_get("/v1/configs/following", &api)
    }

    fn get_config_by_hash(&self, config_hash: Hash) -> IronResult<Response> {
        let hash_str = serde_json::to_string(&config_hash).unwrap().replace(
            "\"",
            "",
        );
        self.get_config_by_hash_str(hash_str)
    }

    fn get_config_by_hash_str<A: AsRef<str>>(&self, hash_str: A) -> IronResult<Response> {
        let api = self.obtain_test_api(ValidatorId::zero());
        request_get(format!("/v1/configs/{}", hash_str.as_ref()), &api)
    }

    fn get_config_votes(&self, config_hash: Hash) -> IronResult<Response> {
        let hash_str = serde_json::to_string(&config_hash).unwrap().replace(
            "\"",
            "",
        );
        self.get_config_votes_by_str(hash_str)
    }

    fn get_config_votes_by_str<A: AsRef<str>>(&self, hash_str: A) -> IronResult<Response> {
        let api = self.obtain_test_api(ValidatorId::zero());
        request_get(format!("/v1/configs/{}/votes", hash_str.as_ref()), &api)
    }

    fn post_config_propose<T: Serialize>(
        &self,
        validator_id: ValidatorId,
        config: T,
    ) -> IronResult<Response> {
        let api = self.obtain_test_api(validator_id);
        request_post_body("/v1/configs/postpropose", config, &api)
    }

    fn post_config_vote<T: Serialize>(
        &self,
        validator_id: ValidatorId,
        config_hash: Hash,
        body: T,
    ) -> IronResult<Response> {
        let hash_str = serde_json::to_string(&config_hash).unwrap().replace(
            "\"",
            "",
        );
        self.post_config_vote_by_str(validator_id, hash_str, body)
    }

    fn post_config_vote_by_str<T: Serialize, A: AsRef<str>>(
        &self,
        validator_id: ValidatorId,
        hash_str: A,
        body: T,
    ) -> IronResult<Response> {
        let api = self.obtain_test_api(validator_id);
        request_post_body(
            format!("/v1/configs/{}/postvote", hash_str.as_ref()),
            body,
            &api,
        )
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
    let sand_cfg = api_sandbox.sandbox.cfg();
    let expected_body = ApiResponseConfigHashInfo {
        hash: sand_cfg.hash(),
        config: sand_cfg,
    };

    let resp_actual_config = api_sandbox.get_actual_config().unwrap();
    let actual_body = response_body(resp_actual_config);
    assert_eq!(actual_body, serde_json::to_value(expected_body).unwrap());
}

#[test]
fn test_get_following_config() {
    let _ = init_logger();
    let api_sandbox = ConfigurationApiSandbox::new();
    let mut rng = thread_rng();
    let initial_cfg = api_sandbox.sandbox.cfg();

    let string_len = rng.gen_range(20u8, 255u8);
    let cfg_name: String = rng.gen_ascii_chars().take(string_len as usize).collect();
    let following_cfg = generate_config_with_message(
        initial_cfg.hash(),
        Height(10),
        &cfg_name,
        &api_sandbox.sandbox,
    );

    {
        api_sandbox
            .post_config_propose(ValidatorId::zero(), following_cfg.clone())
            .unwrap();
        api_sandbox.commit();
    }
    {
        let n_validators = api_sandbox.sandbox.n_validators();
        (0..api_sandbox.sandbox.majority_count(n_validators))
            .inspect(|validator_id| {
                let validator_id = ValidatorId((*validator_id) as u16);
                api_sandbox
                    .post_config_vote(validator_id, following_cfg.hash(), validator_id)
                    .unwrap();
            })
            .collect::<Vec<_>>();
        api_sandbox.commit();
    }

    let expected_body = ApiResponseConfigHashInfo {
        hash: following_cfg.hash(),
        config: following_cfg,
    };

    let resp_following_config = api_sandbox.get_following_config().unwrap();
    let actual_body = response_body(resp_following_config);
    assert_eq!(actual_body, serde_json::to_value(expected_body).unwrap());
}

#[test]
fn test_get_config_by_hash1() {
    let _ = init_logger();
    let api_sandbox = ConfigurationApiSandbox::new();
    let initial_cfg = api_sandbox.sandbox.cfg();

    let expected_body = ApiResponseConfigInfo {
        committed_config: Some(initial_cfg.clone()),
        propose: None,
    };

    let resp_config_by_hash = api_sandbox.get_config_by_hash(initial_cfg.hash()).unwrap();
    let actual_body = response_body(resp_config_by_hash);
    assert_eq!(actual_body, serde_json::to_value(expected_body).unwrap());
}

#[test]
fn test_get_config_by_hash2() {
    let _ = init_logger();
    let api_sandbox = ConfigurationApiSandbox::new();
    let mut rng = thread_rng();
    let initial_cfg = api_sandbox.sandbox.cfg();

    let string_len = rng.gen_range(20u8, 255u8);
    let cfg_name: String = rng.gen_ascii_chars().take(string_len as usize).collect();
    let following_cfg = generate_config_with_message(
        initial_cfg.hash(),
        Height(10),
        &cfg_name,
        &api_sandbox.sandbox,
    );

    let proposer = ValidatorId::zero();
    {
        api_sandbox
            .post_config_propose(proposer, following_cfg.clone())
            .unwrap();
        api_sandbox.commit();
    }

    let expected_body = {
        let expected_hash = {
            let mut fork = MemoryDB::new().fork();
            let mut hashes = ProofListIndex::new(Vec::new(), &mut fork);
            for _ in 0..api_sandbox.sandbox.n_validators() {
                hashes.push(ZEROVOTE.clone());
            }
            hashes.root_hash()
        };
        let (pub_key, sec_key) = (
            api_sandbox.sandbox.service_public_key(proposer),
            api_sandbox.sandbox.service_secret_key(proposer).clone(),
        );
        let expected_propose = TxConfigPropose::new(
            &pub_key,
            str::from_utf8(following_cfg.clone().into_bytes().as_slice())
                .unwrap(),
            &sec_key,
        );
        let expected_voting_data = StorageValueConfigProposeData::new(
            expected_propose,
            &expected_hash,
            api_sandbox.sandbox.n_validators() as u64,
        );
        ApiResponseConfigInfo {
            committed_config: None,
            propose: Some(expected_voting_data),
        }
    };

    let resp_config_by_hash = api_sandbox
        .get_config_by_hash(following_cfg.hash())
        .unwrap();
    let actual_body = response_body(resp_config_by_hash);
    assert_eq!(actual_body, serde_json::to_value(expected_body).unwrap());
}

#[test]
fn test_get_config_by_hash3() {
    let _ = init_logger();
    let api_sandbox = ConfigurationApiSandbox::new();
    let mut rng = thread_rng();
    let initial_cfg = api_sandbox.sandbox.cfg();

    let string_len = rng.gen_range(20u8, 255u8);
    let cfg_name: String = rng.gen_ascii_chars().take(string_len as usize).collect();
    let following_cfg = generate_config_with_message(
        initial_cfg.hash(),
        Height(10),
        &cfg_name,
        &api_sandbox.sandbox,
    );

    let proposer = ValidatorId::zero();
    {
        api_sandbox
            .post_config_propose(proposer, following_cfg.clone())
            .unwrap();
        api_sandbox.commit();
    }
    let votes = {
        let n_validators = api_sandbox.sandbox.n_validators();
        let excluded_validator = 2;
        let votes = (0..api_sandbox.sandbox.majority_count(n_validators) + 1)
            .inspect(|validator_id| if *validator_id != excluded_validator {
                let validator_id = ValidatorId((*validator_id) as u16);
                api_sandbox
                    .post_config_vote(validator_id, following_cfg.hash(), validator_id)
                    .unwrap();
            })
            .map(|validator_id| if validator_id == excluded_validator {
                ZEROVOTE.clone()
            } else {
                let validator_id = ValidatorId(validator_id as u16);
                let (pub_key, sec_key) =
                    (
                        api_sandbox.sandbox.service_public_key(validator_id),
                        api_sandbox.sandbox.service_secret_key(validator_id).clone(),
                    );
                TxConfigVote::new(&pub_key, &following_cfg.hash(), &sec_key)
            })
            .collect::<Vec<_>>();
        api_sandbox.commit();
        votes
    };
    let expected_body = {
        let expected_hash = {
            let mut fork = MemoryDB::new().fork();
            let mut hashes = ProofListIndex::new(Vec::new(), &mut fork);
            hashes.extend(votes);
            hashes.root_hash()
        };
        let (pub_key, sec_key) = (
            api_sandbox.sandbox.service_public_key(proposer),
            api_sandbox.sandbox.service_secret_key(proposer).clone(),
        );
        let expected_propose = TxConfigPropose::new(
            &pub_key,
            str::from_utf8(following_cfg.clone().into_bytes().as_slice())
                .unwrap(),
            &sec_key,
        );
        let expected_voting_data = StorageValueConfigProposeData::new(
            expected_propose,
            &expected_hash,
            api_sandbox.sandbox.n_validators() as u64,
        );
        ApiResponseConfigInfo {
            committed_config: Some(following_cfg.clone()),
            propose: Some(expected_voting_data),
        }
    };

    let resp_config_by_hash = api_sandbox
        .get_config_by_hash(following_cfg.hash())
        .unwrap();
    let actual_body = response_body(resp_config_by_hash);
    assert_eq!(actual_body, serde_json::to_value(expected_body).unwrap());
}

#[test]
fn test_get_config_votes() {
    let _ = init_logger();
    let api_sandbox = ConfigurationApiSandbox::new();
    let mut rng = thread_rng();
    let initial_cfg = api_sandbox.sandbox.cfg();

    let string_len = rng.gen_range(20u8, 255u8);
    let cfg_name: String = rng.gen_ascii_chars().take(string_len as usize).collect();
    let following_cfg = generate_config_with_message(
        initial_cfg.hash(),
        Height(10),
        &cfg_name,
        &api_sandbox.sandbox,
    );

    let expected_body: ApiResponseVotesInfo = None;
    let resp_config_votes = api_sandbox.get_config_votes(following_cfg.hash()).unwrap();
    let actual_body = response_body(resp_config_votes);
    assert_eq!(actual_body, serde_json::to_value(expected_body).unwrap());

    let proposer = ValidatorId::zero();
    {
        api_sandbox
            .post_config_propose(proposer, following_cfg.clone())
            .unwrap();
        api_sandbox.commit();
    }

    let expected_body: ApiResponseVotesInfo = {
        let n_validators = api_sandbox.sandbox.n_validators();
        Some(vec![None; n_validators])
    };
    let resp_config_votes = api_sandbox.get_config_votes(following_cfg.hash()).unwrap();
    let actual_body = response_body(resp_config_votes);
    assert_eq!(actual_body, serde_json::to_value(expected_body).unwrap());

    let votes = {
        let n_validators = api_sandbox.sandbox.n_validators();
        let excluded_validator = 2;
        let votes = (0..api_sandbox.sandbox.majority_count(n_validators) + 1)
            .inspect(|validator_id| if *validator_id != excluded_validator {
                let validator_id = ValidatorId((*validator_id) as u16);
                api_sandbox
                    .post_config_vote(validator_id, following_cfg.hash(), validator_id)
                    .unwrap();
            })
            .map(|validator_id| if validator_id == excluded_validator {
                None
            } else {
                let validator_id = ValidatorId(validator_id as u16);
                let (pub_key, sec_key) =
                    (
                        api_sandbox.sandbox.service_public_key(validator_id),
                        api_sandbox.sandbox.service_secret_key(validator_id).clone(),
                    );
                Some(TxConfigVote::new(&pub_key, &following_cfg.hash(), &sec_key))
            })
            .collect::<Vec<_>>();
        api_sandbox.commit();
        votes
    };
    let expected_body = Some(votes);

    let resp_config_votes = api_sandbox.get_config_votes(following_cfg.hash()).unwrap();
    let actual_body = response_body(resp_config_votes);
    assert_eq!(actual_body, serde_json::to_value(expected_body).unwrap());
}

#[test]
fn test_post_propose_response() {
    let _ = init_logger();
    let api_sandbox = ConfigurationApiSandbox::new();
    let mut rng = thread_rng();
    let initial_cfg = api_sandbox.sandbox.cfg();

    let string_len = rng.gen_range(20u8, 255u8);
    let cfg_name: String = rng.gen_ascii_chars().take(string_len as usize).collect();
    let following_cfg = generate_config_with_message(
        initial_cfg.hash(),
        Height(10),
        &cfg_name,
        &api_sandbox.sandbox,
    );
    let proposer = ValidatorId::zero();
    let (pub_key, sec_key) = (
        api_sandbox.sandbox.service_public_key(proposer),
        api_sandbox.sandbox.service_secret_key(proposer).clone(),
    );
    let expected_body = {
        let propose_tx = TxConfigPropose::new(
            &pub_key,
            str::from_utf8(following_cfg.clone().into_bytes().as_slice()).unwrap(),
            &sec_key,
        );
        ApiResponseProposePost {
            tx_hash: Message::hash(&propose_tx),
            cfg_hash: following_cfg.hash(),
        }
    };

    let resp_config_post = api_sandbox
        .post_config_propose(proposer, following_cfg.clone())
        .unwrap();
    let actual_body = response_body(resp_config_post);
    assert_eq!(actual_body, serde_json::to_value(expected_body).unwrap());
}

#[test]
fn test_post_vote_response() {
    let _ = init_logger();
    let api_sandbox = ConfigurationApiSandbox::new();
    let initial_cfg = api_sandbox.sandbox.cfg();

    let following_cfg = generate_config_with_message(
        initial_cfg.hash(),
        Height(10),
        "config which is voted for",
        &api_sandbox.sandbox,
    );
    let voter = ValidatorId::zero();
    let (pub_key, sec_key) = (
        api_sandbox.sandbox.service_public_key(voter),
        api_sandbox.sandbox.service_secret_key(voter).clone(),
    );
    let expected_body = {
        let vote_tx = TxConfigVote::new(&pub_key, &following_cfg.hash(), &sec_key);
        ApiResponseVotePost { tx_hash: Message::hash(&vote_tx) }
    };

    let resp_config_post = api_sandbox
        .post_config_vote(voter, following_cfg.hash(), voter)
        .unwrap();
    let actual_body = response_body(resp_config_post);
    assert_eq!(actual_body, serde_json::to_value(expected_body).unwrap());
}

fn assert_response_status(
    response: IronResult<Response>,
    expected_status: Status,
    expected_message: &str,
) {
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
