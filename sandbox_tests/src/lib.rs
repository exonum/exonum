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

#![allow(dead_code)]

#[cfg(test)]
#[macro_use]
extern crate log;
extern crate exonum;
extern crate sandbox;
#[cfg(test)]
extern crate exonum_configuration;
#[cfg(test)]
extern crate iron;
#[cfg(test)]
extern crate router;
#[cfg(test)]
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[cfg(test)]
extern crate rand;

use serde_json::Value;

use std::collections::BTreeMap;

use exonum::crypto::Hash;
use exonum::blockchain::config::StoredConfiguration;
use exonum::blockchain::Service;
use exonum::encoding::serialize::json::reexport as serde_json;
use exonum::helpers::Height;
use sandbox::sandbox::Sandbox;
use sandbox::timestamping::TimestampingService;

#[derive(Serialize)]
struct CfgStub {
    cfg_string: String,
}

fn generate_config_with_message(
    prev_cfg_hash: Hash,
    actual_from: Height,
    timestamping_service_cfg_message: &str,
    sandbox: &Sandbox,
) -> StoredConfiguration {
    let mut services: BTreeMap<String, Value> = BTreeMap::new();
    let tmstmp_id = TimestampingService::new().service_id();
    let service_cfg = CfgStub { cfg_string: timestamping_service_cfg_message.to_string() };
    services.insert(
        format!("{}", tmstmp_id),
        serde_json::to_value(service_cfg).unwrap(),
    );
    StoredConfiguration {
        previous_cfg_hash: prev_cfg_hash,
        actual_from: actual_from,
        validator_keys: sandbox.nodes_keys(),
        consensus: sandbox.cfg().consensus,
        services: services,
    }
}

#[cfg(test)]
mod api_tests;

#[cfg(test)]
mod tests {
    use serde_json::{self, Value};

    use std::collections::BTreeMap;
    use std::str;

    use exonum::crypto::{Hash, Seed, SEED_LENGTH, HASH_SIZE, gen_keypair_from_seed, hash};
    use exonum::blockchain::config::{StoredConfiguration, ValidatorKeys};
    use exonum::storage::StorageValue;
    use exonum::messages::{Message, FromRaw};
    use exonum::helpers::{Height, Round, ValidatorId};
    use sandbox::timestamping::TimestampingService;
    use sandbox::sandbox::Sandbox;
    use sandbox::sandbox_with_services;
    use sandbox::sandbox_tests_helper::{SandboxState, add_one_height_with_transactions,
                                        add_one_height_with_transactions_from_other_validator};
    use exonum_configuration::{TxConfigPropose, TxConfigVote, ConfigurationService,
                               ConfigurationSchema};

    use super::generate_config_with_message;

    pub fn configuration_sandbox() -> (Sandbox, SandboxState, StoredConfiguration) {
        use exonum;
        let _ = exonum::helpers::init_logger();
        let sandbox = sandbox_with_services(vec![
            Box::new(TimestampingService::new()),
            Box::new(ConfigurationService::new()),
        ]);
        let sandbox_state = SandboxState::new();
        let initial_cfg = sandbox.cfg();
        (sandbox, sandbox_state, initial_cfg)
    }

    fn get_propose(sandbox: &Sandbox, config_hash: Hash) -> Option<TxConfigPropose> {
        let snapshot = sandbox.blockchain_ref().snapshot();
        let schema = ConfigurationSchema::new(&snapshot);
        schema.get_propose(&config_hash)
    }

    fn get_votes_for_propose(sandbox: &Sandbox, config_hash: Hash) -> Vec<Option<TxConfigVote>> {
        let snapshot = sandbox.blockchain_ref().snapshot();
        let schema = ConfigurationSchema::new(&snapshot);
        schema.get_votes(&config_hash)
    }

    #[test]
    fn test_full_node_to_validator() {
        use super::CfgStub;
        use exonum::blockchain::Service;
        use serde_json::Value;
        let (sandbox, sandbox_state, initial_cfg) = configuration_sandbox();

        sandbox.assert_state(Height(1), Round::first());
        add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
        sandbox.assert_state(Height(2), Round::first());

        assert_eq!(sandbox.is_validator(), true);

        let nodes_keys = sandbox.nodes_keys();

        let mut services: BTreeMap<String, Value> = BTreeMap::new();
        let tmstmp_id = TimestampingService::new().service_id();
        let service_cfg = CfgStub { cfg_string: "some test".to_string() };
        services.insert(
            format!("{}", tmstmp_id),
            serde_json::to_value(service_cfg).unwrap(),
        );

        let full_node_cfg = StoredConfiguration {
            previous_cfg_hash: initial_cfg.hash(),
            actual_from: Height(4),
            validator_keys: nodes_keys[1..].to_vec(),
            consensus: sandbox.cfg().consensus,
            services: services.clone(),
        };



        {
            let propose_tx = TxConfigPropose::new(
                &sandbox.service_public_key(ValidatorId(1)),
                str::from_utf8(full_node_cfg.clone().into_bytes().as_slice()).unwrap(),
                sandbox.service_secret_key(ValidatorId(1)),
            );
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[propose_tx.raw().clone()]);
        }
        {
            let mut votes = Vec::new();
            for validator in 0..3 {
                let validator = ValidatorId(validator);
                votes.push(
                    TxConfigVote::new(
                        &sandbox.service_public_key(validator),
                        &full_node_cfg.hash(),
                        sandbox.service_secret_key(validator),
                    ).raw()
                        .clone(),
                );
            }
            add_one_height_with_transactions(&sandbox, &sandbox_state, &votes);
        }
        sandbox.assert_state(Height(4), Round::first());
        assert_eq!(full_node_cfg, sandbox.cfg());
        assert_eq!(sandbox.is_validator(), false);

        let validator_cfg = StoredConfiguration {
            previous_cfg_hash: full_node_cfg.hash(),
            actual_from: Height(6),
            validator_keys: nodes_keys[0..].to_vec(),
            consensus: sandbox.cfg().consensus,
            services: services.clone(),
        };

        {
            let propose_tx = TxConfigPropose::new(
                &sandbox.service_public_key(ValidatorId(1)),
                str::from_utf8(validator_cfg.clone().into_bytes().as_slice()).unwrap(),
                sandbox.service_secret_key(ValidatorId(1)),
            );
            add_one_height_with_transactions_from_other_validator(
                &sandbox,
                &sandbox_state,
                &[propose_tx.raw().clone()],
            );
        }
        {
            let mut votes = Vec::new();
            for validator in 0..3 {
                let validator = ValidatorId(validator);
                votes.push(
                    TxConfigVote::new(
                        &sandbox.service_public_key(validator),
                        &validator_cfg.hash(),
                        sandbox.service_secret_key(validator),
                    ).raw()
                        .clone(),
                );
            }
            add_one_height_with_transactions_from_other_validator(&sandbox, &sandbox_state, &votes);
        }
        sandbox.assert_state(Height(6), Round::first());
        assert_eq!(validator_cfg, sandbox.cfg());
        assert_eq!(sandbox.is_validator(), true);

    }

    #[test]
    fn test_add_validators_to_config() {
        let (mut sandbox, sandbox_state, initial_cfg) = configuration_sandbox();
        let start_time = sandbox.time();
        sandbox.assert_state(Height(1), Round::first());

        let new_validator_keypairs: Vec<_> = (40..44)
            .map(|seed_num| {
                gen_keypair_from_seed(&Seed::new([seed_num; SEED_LENGTH]))
            })
            .collect();
        let new_service_keypairs: Vec<_> = (40..44)
            .map(|seed_num| {
                gen_keypair_from_seed(&Seed::new([seed_num * 2; SEED_LENGTH]))
            })
            .collect();

        let mut nodes_keys = sandbox.nodes_keys();
        let old_len = nodes_keys.len();
        nodes_keys.extend(
            new_validator_keypairs
                .iter()
                .zip(new_service_keypairs.iter())
                .map(|(ck, sk)| {
                    ValidatorKeys {
                        consensus_key: ck.0,
                        service_key: sk.0,
                    }
                }),
        );
        let new_len = nodes_keys.len();

        let actual_from = Height(3);
        let services: BTreeMap<String, Value> = BTreeMap::new();
        let added_keys_cfg = StoredConfiguration {
            previous_cfg_hash: initial_cfg.hash(),
            actual_from: actual_from,
            validator_keys: nodes_keys,
            consensus: sandbox.cfg().consensus,
            services: services,
        };
        {
            let propose_tx = TxConfigPropose::new(
                &sandbox.service_public_key(ValidatorId(1)),
                str::from_utf8(added_keys_cfg.clone().into_bytes().as_slice())
                    .unwrap(),
                sandbox.service_secret_key(ValidatorId(1)),
            );
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[propose_tx.raw().clone()]);
            sandbox.assert_state(Height(2), Round::first());
        }
        {
            let mut votes = Vec::new();
            for validator in 0..3 {
                let validator = ValidatorId(validator);
                votes.push(
                    TxConfigVote::new(
                        &sandbox.service_public_key(validator),
                        &added_keys_cfg.hash(),
                        sandbox.service_secret_key(validator),
                    ).raw()
                        .clone(),
                );
            }
            add_one_height_with_transactions(&sandbox, &sandbox_state, &votes);
            sandbox.assert_state(Height(3), Round::first());
        }
        {
            let consensus_keys = new_validator_keypairs.iter().cloned().collect();
            let service_keys = new_service_keypairs.iter().cloned().collect();
            sandbox.set_validators_map(new_len as u8, consensus_keys, service_keys);
            sandbox.initialize(start_time, old_len, new_len);
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
            sandbox.assert_state(Height(4), Round::first());
        }
        let new_cfg =
            generate_config_with_message(added_keys_cfg.hash(), Height(8), "First cfg", &sandbox);
        {
            let propose_tx = TxConfigPropose::new(
                &sandbox.service_public_key(ValidatorId(1)),
                str::from_utf8(new_cfg.clone().into_bytes().as_slice()).unwrap(),
                sandbox.service_secret_key(ValidatorId(1)),
            );
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[propose_tx.raw().clone()]);
            sandbox.assert_state(Height(5), Round::first());
        }
        {
            let mut votes = Vec::new();
            for validator in 0..5 {
                let validator = ValidatorId(validator);
                votes.push(
                    TxConfigVote::new(
                        &sandbox.service_public_key(validator),
                        &new_cfg.hash(),
                        sandbox.service_secret_key(validator),
                    ).raw()
                        .clone(),
                );
            }
            add_one_height_with_transactions(&sandbox, &sandbox_state, &votes);
            sandbox.assert_state(Height(6), Round::first());
            assert_eq!(None, sandbox.following_cfg());
        }
        {
            let validator_5 = ValidatorId(5);
            let last_vote = TxConfigVote::new(
                &sandbox.service_public_key(validator_5),
                &new_cfg.hash(),
                sandbox.service_secret_key(validator_5),
            );
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[last_vote.raw().clone()]);
            sandbox.assert_state(Height(7), Round::first());
            assert_eq!(Some(new_cfg), sandbox.following_cfg());
        }
    }

    #[test]
    fn test_exclude_sandbox_node_from_config() {
        let (sandbox, sandbox_state, initial_cfg) = configuration_sandbox();
        sandbox.assert_state(Height(1), Round::first());

        let new_public_keys = (40..44)
            .map(|seed| {
                (
                    gen_keypair_from_seed(&Seed::new([seed; SEED_LENGTH])).0,
                    gen_keypair_from_seed(&Seed::new([seed * 2; SEED_LENGTH])).0,
                )
            })
            .collect::<Vec<_>>();

        let actual_from = Height(3);
        let services: BTreeMap<String, Value> = BTreeMap::new();
        let excluding_cfg = StoredConfiguration {
            previous_cfg_hash: initial_cfg.hash(),
            actual_from: actual_from,
            validator_keys: new_public_keys
                .iter()
                .map(|x| {
                    ValidatorKeys {
                        consensus_key: x.0,
                        service_key: x.1,
                    }
                })
                .collect(),
            consensus: sandbox.cfg().consensus,
            services: services,
        };
        {
            let propose_tx = TxConfigPropose::new(
                &sandbox.service_public_key(ValidatorId(1)),
                str::from_utf8(excluding_cfg.clone().into_bytes().as_slice()).unwrap(),
                sandbox.service_secret_key(ValidatorId(1)),
            );
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[propose_tx.raw().clone()]);
            sandbox.assert_state(Height(2), Round::first());
        }
        {
            let mut votes = Vec::new();
            for validator in 0..3 {
                let validator = ValidatorId(validator);
                votes.push(
                    TxConfigVote::new(
                        &sandbox.service_public_key(validator),
                        &excluding_cfg.hash(),
                        sandbox.service_secret_key(validator),
                    ).raw()
                        .clone(),
                );
            }
            add_one_height_with_transactions(&sandbox, &sandbox_state, &votes);
            sandbox.assert_state(Height(3), Round::first());
        }
    }

    #[test]
    fn test_discard_propose_for_same_cfg() {
        let (sandbox, sandbox_state, initial_cfg) = configuration_sandbox();
        sandbox.assert_state(Height(1), Round::first());
        let new_cfg =
            generate_config_with_message(initial_cfg.hash(), Height(4), "First cfg", &sandbox);
        let propose_tx = TxConfigPropose::new(
            &sandbox.service_public_key(ValidatorId(1)),
            str::from_utf8(new_cfg.clone().into_bytes().as_slice()).unwrap(),
            sandbox.service_secret_key(ValidatorId(1)),
        );
        {
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[propose_tx.raw().clone()]);
            sandbox.assert_state(Height(2), Round::first());
            assert_eq!(
                Some(propose_tx.clone()),
                get_propose(&sandbox, new_cfg.hash())
            );
        }
        {
            let validator_0 = ValidatorId::zero();
            let duplicate_cfg_propose = TxConfigPropose::new(
                &sandbox.service_public_key(validator_0),
                str::from_utf8(new_cfg.clone().into_bytes().as_slice())
                    .unwrap(),
                sandbox.service_secret_key(validator_0),
            );
            add_one_height_with_transactions(
                &sandbox,
                &sandbox_state,
                &[duplicate_cfg_propose.raw().clone()],
            );
            sandbox.assert_state(Height(3), Round::first());
            assert_eq!(Some(propose_tx), get_propose(&sandbox, new_cfg.hash()));
        }
    }

    #[test]
    fn test_discard_vote_for_absent_propose() {
        let (sandbox, sandbox_state, initial_cfg) = configuration_sandbox();
        sandbox.assert_state(Height(1), Round::first());
        let new_cfg =
            generate_config_with_message(initial_cfg.hash(), Height(4), "First cfg", &sandbox);
        let absent_cfg =
            generate_config_with_message(initial_cfg.hash(), Height(4), "Absent propose", &sandbox);
        {
            let validator_1 = ValidatorId(1);
            let propose_tx = TxConfigPropose::new(
                &sandbox.service_public_key(validator_1),
                str::from_utf8(new_cfg.clone().into_bytes().as_slice()).unwrap(),
                sandbox.service_secret_key(validator_1),
            );
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[propose_tx.raw().clone()]);
            sandbox.assert_state(Height(2), Round::first());
        }
        {
            let validator_3 = ValidatorId(3);
            let legal_vote = TxConfigVote::new(
                &sandbox.service_public_key(validator_3),
                &new_cfg.hash(),
                sandbox.service_secret_key(validator_3),
            );
            let illegal_vote = TxConfigVote::new(
                &sandbox.service_public_key(validator_3),
                &absent_cfg.hash(),
                sandbox.service_secret_key(validator_3),
            );
            add_one_height_with_transactions(
                &sandbox,
                &sandbox_state,
                &[legal_vote.raw().clone(), illegal_vote.raw().clone()],
            );
            sandbox.assert_state(Height(3), Round::first());
            let votes = get_votes_for_propose(&sandbox, new_cfg.hash());
            assert!(votes.contains(&Some(legal_vote)));
            assert!(!votes.contains(&Some(illegal_vote)));
        }
    }

    #[test]
    fn test_discard_proposes_with_expired_actual_from() {
        let (sandbox, sandbox_state, initial_cfg) = configuration_sandbox();

        let target_height = 10;
        {
            for _ in 1..target_height {
                add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
            }
            sandbox.assert_state(Height(target_height), Round::first());
        }
        let new_cfg = generate_config_with_message(
            initial_cfg.hash(),
            Height(target_height),
            "First cfg",
            &sandbox,
        );
        {
            let propose_tx = TxConfigPropose::new(
                &sandbox.service_public_key(ValidatorId(1)),
                str::from_utf8(new_cfg.clone().into_bytes().as_slice()).unwrap(),
                sandbox.service_secret_key(ValidatorId(1)),
            );
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[propose_tx.raw().clone()]);
            sandbox.assert_state(Height(target_height).next(), Round::first());
            assert_eq!(None, get_propose(&sandbox, new_cfg.hash()));
        }
    }

    #[test]
    fn test_discard_votes_with_expired_actual_from() {
        let (sandbox, sandbox_state, initial_cfg) = configuration_sandbox();
        sandbox.assert_state(Height(1), Round::first());
        let target_height = 10;

        let new_cfg = generate_config_with_message(
            initial_cfg.hash(),
            Height(target_height),
            "First cfg",
            &sandbox,
        );
        {
            let validator_1 = ValidatorId(1);
            let propose_tx = TxConfigPropose::new(
                &sandbox.service_public_key(validator_1),
                str::from_utf8(new_cfg.clone().into_bytes().as_slice()).unwrap(),
                sandbox.service_secret_key(validator_1),
            );
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[propose_tx.raw().clone()]);
            sandbox.assert_state(Height(2), Round::first());
            assert_eq!(Some(propose_tx), get_propose(&sandbox, new_cfg.hash()));
        }
        {
            let validator_3 = ValidatorId(3);
            let legal_vote = TxConfigVote::new(
                &sandbox.service_public_key(validator_3),
                &new_cfg.hash(),
                sandbox.service_secret_key(validator_3),
            );
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[legal_vote.raw().clone()]);
            sandbox.assert_state(Height(3), Round::first());
            let votes = get_votes_for_propose(&sandbox, new_cfg.hash());
            assert!(votes.contains(&Some(legal_vote)));
        }
        {
            for _ in 3..target_height {
                add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
            }
            sandbox.assert_state(Height(target_height), Round::first());
        }
        {
            let validator_0 = ValidatorId::zero();
            let illegal_vote = TxConfigVote::new(
                &sandbox.service_public_key(validator_0),
                &new_cfg.hash(),
                sandbox.service_secret_key(validator_0),
            );
            add_one_height_with_transactions(
                &sandbox,
                &sandbox_state,
                &[illegal_vote.raw().clone()],
            );
            sandbox.assert_state(Height(target_height).next(), Round::first());
            let votes = get_votes_for_propose(&sandbox, new_cfg.hash());
            assert!(!votes.contains(&Some(illegal_vote)));
        }
    }

    #[test]
    fn test_discard_invalid_config_json() {
        let (sandbox, sandbox_state, _) = configuration_sandbox();
        sandbox.assert_state(Height(1), Round::first());
        let cfg_bytes = [70; 74];
        let new_cfg = str::from_utf8(&cfg_bytes).unwrap(); // invalid json bytes
        {
            let propose_tx = TxConfigPropose::new(
                &sandbox.service_public_key(ValidatorId(1)),
                new_cfg,
                sandbox.service_secret_key(ValidatorId(1)),
            );
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[propose_tx.raw().clone()]);
            sandbox.assert_state(Height(2), Round::first());
            assert_eq!(None, get_propose(&sandbox, hash(new_cfg.as_bytes())));
        }
    }

    #[test]
    fn test_change_service_config() {
        let (sandbox, sandbox_state, initial_cfg) = configuration_sandbox();

        let target_height = 10;
        {
            for _ in 0..target_height {
                add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
            }
            sandbox.assert_state(Height(target_height).next(), Round::first());
        }
        let new_cfg = generate_config_with_message(
            initial_cfg.hash(),
            Height(target_height + 4),
            "First cfg",
            &sandbox,
        );
        {
            let propose_tx = TxConfigPropose::new(
                &sandbox.service_public_key(ValidatorId(1)),
                str::from_utf8(new_cfg.clone().into_bytes().as_slice()).unwrap(),
                sandbox.service_secret_key(ValidatorId(1)),
            );
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[propose_tx.raw().clone()]);
            sandbox.assert_state(Height(target_height + 2), Round::first());
            assert_eq!(propose_tx, get_propose(&sandbox, new_cfg.hash()).unwrap());
        }
        {
            let mut expected_votes = Vec::new();
            for validator in 0..2 {
                let validator = ValidatorId(validator);
                expected_votes.push(
                    TxConfigVote::new(
                        &sandbox.service_public_key(validator),
                        &new_cfg.hash(),
                        sandbox.service_secret_key(validator),
                    ).raw()
                        .clone(),
                );
            }
            let validator_2 = ValidatorId(2);
            let unposted_vote = TxConfigVote::new(
                &sandbox.service_public_key(validator_2),
                &new_cfg.hash(),
                sandbox.service_secret_key(validator_2),
            );
            add_one_height_with_transactions(&sandbox, &sandbox_state, &expected_votes);
            sandbox.assert_state(Height(target_height + 3), Round::first());
            let actual_votes = get_votes_for_propose(&sandbox, new_cfg.hash());
            for raw_vote in expected_votes {
                let exp_vote = TxConfigVote::from_raw(raw_vote).unwrap();
                assert!(actual_votes.contains(&Some(exp_vote)));
            }
            assert!(!actual_votes.contains(&Some(unposted_vote)));
            assert_eq!(initial_cfg, sandbox.cfg());
            assert_eq!(None, sandbox.following_cfg());
        }
        {
            let validator_2 = ValidatorId(2);
            let vote3 = TxConfigVote::new(
                &sandbox.service_public_key(validator_2),
                &new_cfg.hash(),
                sandbox.service_secret_key(validator_2),
            );
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[vote3.raw().clone()]);
            sandbox.assert_state(Height(target_height + 4), Round::first());
            let votes = get_votes_for_propose(&sandbox, new_cfg.hash());
            assert!(votes.contains(&Some(vote3)));
            assert_eq!(new_cfg, sandbox.cfg());
        }
    }

    #[test]
    fn test_config_txs_discarded_when_following_config_present() {
        let (sandbox, sandbox_state, initial_cfg) = configuration_sandbox();
        sandbox.assert_state(Height(1), Round::first());

        let following_config = generate_config_with_message(
            initial_cfg.hash(),
            Height(6),
            "Following cfg at height 6",
            &sandbox,
        );

        {
            let propose_tx = TxConfigPropose::new(
                &sandbox.service_public_key(ValidatorId(1)),
                str::from_utf8(following_config.clone().into_bytes().as_slice())
                    .unwrap(),
                sandbox.service_secret_key(ValidatorId(1)),
            );
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[propose_tx.raw().clone()]);
            sandbox.assert_state(Height(2), Round::first());
            assert_eq!(
                Some(propose_tx),
                get_propose(&sandbox, following_config.hash())
            );
        }
        {
            let votes = (0..3)
                .map(|validator| {
                    let validator = ValidatorId(validator);
                    TxConfigVote::new(
                        &sandbox.service_public_key(validator),
                        &following_config.hash(),
                        sandbox.service_secret_key(validator),
                    ).raw()
                        .clone()
                })
                .collect::<Vec<_>>();
            add_one_height_with_transactions(&sandbox, &sandbox_state, &votes);
            sandbox.assert_state(Height(3), Round::first());
            assert_eq!(sandbox.cfg(), initial_cfg);
            assert_eq!(sandbox.following_cfg(), Some(following_config.clone()));
        }
        let new_cfg =
            generate_config_with_message(initial_cfg.hash(), Height(7), "New cfg", &sandbox);

        {
            let propose_tx_new = TxConfigPropose::new(
                &sandbox.service_public_key(ValidatorId(1)),
                str::from_utf8(new_cfg.clone().into_bytes().as_slice()).unwrap(),
                sandbox.service_secret_key(ValidatorId(1)),
            );
            add_one_height_with_transactions(
                &sandbox,
                &sandbox_state,
                &[propose_tx_new.raw().clone()],
            );
            sandbox.assert_state(Height(4), Round::first());

            assert_eq!(None, get_propose(&sandbox, new_cfg.hash()));
        }
        let vote_validator_0 = TxConfigVote::new(
            &sandbox.service_public_key(ValidatorId::zero()),
            &following_config.hash(),
            sandbox.service_secret_key(ValidatorId::zero()),
        );
        let vote_validator_3 = TxConfigVote::new(
            &sandbox.service_public_key(ValidatorId(3)),
            &following_config.hash(),
            sandbox.service_secret_key(ValidatorId(3)),
        );
        {
            add_one_height_with_transactions(
                &sandbox,
                &sandbox_state,
                &[vote_validator_3.raw().clone()],
            );
            sandbox.assert_state(Height(5), Round::first());

            let votes = get_votes_for_propose(&sandbox, following_config.hash());
            assert!(votes.contains(&Some(vote_validator_0)));
            assert!(!votes.contains(&Some(vote_validator_3)));
            assert_eq!(initial_cfg, sandbox.cfg());
        }
        {
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
            sandbox.assert_state(Height(6), Round::first());
            assert_eq!(following_config, sandbox.cfg());
        }
    }

    #[test]
    fn test_config_txs_discarded_when_not_referencing_actual_config_or_sent_by_illegal_validator() {
        let (sandbox, sandbox_state, initial_cfg) = configuration_sandbox();
        sandbox.assert_state(Height(1), Round::first());

        let new_cfg_bad_previous_cfg = generate_config_with_message(
            Hash::new([11; HASH_SIZE]),
            Height(6),
            "Following cfg at height 6",
            &sandbox,
        );
        // not actual config hash

        let new_cfg = generate_config_with_message(
            initial_cfg.hash(),
            Height(6),
            "Following cfg at height 6",
            &sandbox,
        );
        let discarded_votes_cfg = generate_config_with_message(
            initial_cfg.hash(),
            Height(8),
            "discarded votes",
            &sandbox,
        );

        let (illegal_pub, illegal_sec) = gen_keypair_from_seed(&Seed::new([66; 32]));

        {
            let illegal_propose1 = TxConfigPropose::new(
                &sandbox.service_public_key(ValidatorId(1)),
                str::from_utf8(
                    new_cfg_bad_previous_cfg.clone().into_bytes().as_slice(),
                ).unwrap(),
                sandbox.service_secret_key(ValidatorId(1)),
            );
            let illegal_propose2 = TxConfigPropose::new(
                &illegal_pub,
                // not a member of actual config
                str::from_utf8(new_cfg.clone().into_bytes().as_slice()).unwrap(),
                &illegal_sec,
            );
            add_one_height_with_transactions(
                &sandbox,
                &sandbox_state,
                &[
                    illegal_propose1.raw().clone(),
                    illegal_propose2.raw().clone(),
                ],
            );
            sandbox.assert_state(Height(2), Round::first());
            assert_eq!(None, get_propose(&sandbox, new_cfg_bad_previous_cfg.hash()));
            assert_eq!(None, get_propose(&sandbox, new_cfg.hash()));
        }
        {
            let legal_propose1 = TxConfigPropose::new(
                &sandbox.service_public_key(ValidatorId(1)),
                str::from_utf8(new_cfg.clone().into_bytes().as_slice()).unwrap(),
                sandbox.service_secret_key(ValidatorId(1)),
            );
            let legal_propose2 =
                TxConfigPropose::new(
                    &sandbox.service_public_key(ValidatorId(1)),
                    str::from_utf8(discarded_votes_cfg.clone().into_bytes().as_slice()).unwrap(),
                    sandbox.service_secret_key(ValidatorId(1)),
                );
            add_one_height_with_transactions(
                &sandbox,
                &sandbox_state,
                &[legal_propose1.raw().clone(), legal_propose2.raw().clone()],
            );
            sandbox.assert_state(Height(3), Round::first());
            assert_eq!(Some(legal_propose1), get_propose(&sandbox, new_cfg.hash()));
            assert_eq!(
                Some(legal_propose2),
                get_propose(&sandbox, discarded_votes_cfg.hash())
            );
        }
        {
            let illegal_validator_vote =
                TxConfigVote::new(&illegal_pub, &discarded_votes_cfg.hash(), &illegal_sec);
            add_one_height_with_transactions(
                &sandbox,
                &sandbox_state,
                &[illegal_validator_vote.raw().clone()],
            );
            sandbox.assert_state(Height(4), Round::first());
            let votes = get_votes_for_propose(&sandbox, discarded_votes_cfg.hash());
            assert!(!votes.contains(&Some(illegal_validator_vote)));
        }
        {
            let votes = (0..3)
                .map(|validator| {
                    let validator = ValidatorId(validator);
                    TxConfigVote::new(
                        &sandbox.service_public_key(validator),
                        &new_cfg.hash(),
                        sandbox.service_secret_key(validator),
                    ).raw()
                        .clone()
                })
                .collect::<Vec<_>>();
            add_one_height_with_transactions(&sandbox, &sandbox_state, &votes);
            sandbox.assert_state(Height(5), Round::first());
            assert_eq!(initial_cfg, sandbox.cfg());
            assert_eq!(Some(new_cfg.clone()), sandbox.following_cfg());
        }
        {
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
            sandbox.assert_state(Height(6), Round::first());
            assert_eq!(new_cfg, sandbox.cfg());
            assert_eq!(None, sandbox.following_cfg());
        }
        {
            let expected_votes = (0..3)
                .map(|validator| {
                    let validator = ValidatorId(validator);
                    TxConfigVote::new(
                        &sandbox.service_public_key(validator),
                        &discarded_votes_cfg.hash(),
                        sandbox.service_secret_key(validator),
                    ).raw()
                        .clone()
                })
                .collect::<Vec<_>>();
            add_one_height_with_transactions(&sandbox, &sandbox_state, &expected_votes);
            sandbox.assert_state(Height(7), Round::first());
            let actual_votes = get_votes_for_propose(&sandbox, discarded_votes_cfg.hash());
            for raw_vote in expected_votes {
                let exp_vote = TxConfigVote::from_raw(raw_vote).unwrap();
                assert!(!actual_votes.contains(&Some(exp_vote)));
            }
        }
    }

    /// regression: votes' were summed for all proposes simultaneously, and not for the same propose
    #[test]
    fn test_regression_majority_votes_for_different_proposes() {
        let (sandbox, sandbox_state, initial_cfg) = configuration_sandbox();
        sandbox.assert_state(Height(1), Round::first());

        let actual_from = Height(5);

        let new_cfg1 =
            generate_config_with_message(initial_cfg.hash(), actual_from, "First cfg", &sandbox);
        let new_cfg2 =
            generate_config_with_message(initial_cfg.hash(), actual_from, "Second cfg", &sandbox);
        {
            let mut proposes = Vec::new();
            for cfg in &[new_cfg1.clone(), new_cfg2.clone()] {
                proposes.push(
                    TxConfigPropose::new(
                        &sandbox.service_public_key(ValidatorId(1)),
                        str::from_utf8(cfg.clone().into_bytes().as_slice()).unwrap(),
                        sandbox.service_secret_key(ValidatorId(1)),
                    ).raw()
                        .clone(),
                );
            }

            add_one_height_with_transactions(&sandbox, &sandbox_state, &proposes);
            sandbox.assert_state(Height(2), Round::first());
        }
        {
            let mut votes = Vec::new();
            for validator in 0..2 {
                let validator = ValidatorId(validator);
                votes.push(
                    TxConfigVote::new(
                        &sandbox.service_public_key(validator),
                        &new_cfg1.hash(),
                        sandbox.service_secret_key(validator),
                    ).raw()
                        .clone(),
                );
            }

            add_one_height_with_transactions(&sandbox, &sandbox_state, &votes);
            sandbox.assert_state(Height(3), Round::first());
            assert_eq!(initial_cfg, sandbox.cfg());
        }
        {
            let validator_2 = ValidatorId(2);
            let prop2_validator2 = TxConfigVote::new(
                &sandbox.service_public_key(validator_2),
                &new_cfg2.hash(),
                sandbox.service_secret_key(validator_2),
            );

            add_one_height_with_transactions(
                &sandbox,
                &sandbox_state,
                &[prop2_validator2.raw().clone()],
            );
            sandbox.assert_state(Height(4), Round::first());
            assert_eq!(initial_cfg, sandbox.cfg());
        }
        {
            let validator_2 = ValidatorId(2);
            let prop1_validator2 = TxConfigVote::new(
                &sandbox.service_public_key(validator_2),
                &new_cfg1.hash(),
                sandbox.service_secret_key(validator_2),
            );

            add_one_height_with_transactions(
                &sandbox,
                &sandbox_state,
                &[prop1_validator2.raw().clone()],
            );
            sandbox.assert_state(Height(5), Round::first());
            assert_eq!(new_cfg1, sandbox.cfg());
        }
    }

    #[test]
    fn test_regression_new_vote_for_older_config_applies_old_config() {
        let (sandbox, sandbox_state, initial_cfg) = configuration_sandbox();
        sandbox.assert_state(Height(1), Round::first());

        let new_cfg1 =
            generate_config_with_message(initial_cfg.hash(), Height(3), "First cfg", &sandbox);
        let new_cfg2 =
            generate_config_with_message(new_cfg1.hash(), Height(5), "Second cfg", &sandbox);

        {
            let propose_tx1 = TxConfigPropose::new(
                &sandbox.service_public_key(ValidatorId(1)),
                str::from_utf8(new_cfg1.clone().into_bytes().as_slice()).unwrap(),
                sandbox.service_secret_key(ValidatorId(1)),
            );

            add_one_height_with_transactions(
                &sandbox,
                &sandbox_state,
                &[propose_tx1.raw().clone()],
            );
            sandbox.assert_state(Height(2), Round::first());
        }
        {
            let mut votes_for_new_cfg1 = Vec::new();
            for validator in 0..3 {
                let validator = ValidatorId(validator);
                votes_for_new_cfg1.push(
                    TxConfigVote::new(
                        &sandbox.service_public_key(validator),
                        &new_cfg1.hash(),
                        sandbox.service_secret_key(validator),
                    ).raw()
                        .clone(),
                );
            }
            add_one_height_with_transactions(&sandbox, &sandbox_state, &votes_for_new_cfg1);
            sandbox.assert_state(Height(3), Round::first());
            assert_eq!(new_cfg1, sandbox.cfg());
        }
        {
            let propose_tx2 = TxConfigPropose::new(
                &sandbox.service_public_key(ValidatorId(1)),
                str::from_utf8(new_cfg2.clone().into_bytes().as_slice()).unwrap(),
                sandbox.service_secret_key(ValidatorId(1)),
            );

            add_one_height_with_transactions(
                &sandbox,
                &sandbox_state,
                &[propose_tx2.raw().clone()],
            );
            sandbox.assert_state(Height(4), Round::first());
        }
        {
            let mut votes_for_new_cfg2 = Vec::new();
            for validator in 0..3 {
                let validator = ValidatorId(validator);
                votes_for_new_cfg2.push(
                    TxConfigVote::new(
                        &sandbox.service_public_key(validator),
                        &new_cfg2.hash(),
                        sandbox.service_secret_key(validator),
                    ).raw()
                        .clone(),
                );
            }
            add_one_height_with_transactions(&sandbox, &sandbox_state, &votes_for_new_cfg2);
            sandbox.assert_state(Height(5), Round::first());
            assert_eq!(new_cfg2, sandbox.cfg());
        }
        {
            let validator_3 = ValidatorId(3);
            let prop1_validator3 = TxConfigVote::new(
                &sandbox.service_public_key(validator_3),
                &new_cfg1.hash(),
                sandbox.service_secret_key(validator_3),
            );
            add_one_height_with_transactions(
                &sandbox,
                &sandbox_state,
                &[prop1_validator3.raw().clone()],
            );
            sandbox.assert_state(Height(6), Round::first());
            assert_eq!(new_cfg2, sandbox.cfg());
        }
    }
}
