#![allow(dead_code)]

#[macro_use]
extern crate log;
extern crate exonum;
extern crate blockchain_explorer;
extern crate sandbox;
extern crate configuration_service;
extern crate iron;
extern crate router;
extern crate serde_json;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate rand;

use std::collections::BTreeMap;
use serde_json::Value;
use serde_json::value::ToJson;
use exonum::crypto::Hash;
use exonum::blockchain::config::StoredConfiguration;
use exonum::blockchain::Service;
use sandbox::sandbox::Sandbox;
use sandbox::timestamping::TimestampingService;

#[derive(Serialize)]
struct CfgStub {
    cfg_string: String,
}

fn generate_config_with_message(prev_cfg_hash: Hash,
                                actual_from: u64,
                                timestamping_service_cfg_message: &str,
                                sandbox: &Sandbox)
                                -> StoredConfiguration {
    let mut services: BTreeMap<String, Value> = BTreeMap::new();
    let tmstmp_id = TimestampingService::new().service_id();
    let service_cfg = CfgStub { cfg_string: timestamping_service_cfg_message.to_string() };
    services.insert(format!("{}", tmstmp_id), service_cfg.to_json());
    StoredConfiguration {
        previous_cfg_hash: prev_cfg_hash,
        actual_from: actual_from,
        validators: sandbox.validators(),
        consensus: sandbox.cfg().consensus,
        services: services,
    }
}

#[cfg(test)]
mod api_tests;

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use super::generate_config_with_message;
    use exonum::crypto::{Hash, Seed, SEED_LENGTH, HASH_SIZE, gen_keypair_from_seed, hash};
    use exonum::blockchain::config::StoredConfiguration;
    use exonum::storage::{StorageValue, Error as StorageError};
    use exonum::messages::{Message, FromRaw};
    use sandbox::timestamping::TimestampingService;
    use sandbox::sandbox::Sandbox;
    use sandbox::sandbox_with_services;
    use sandbox::sandbox_tests_helper::{SandboxState, add_one_height_with_transactions,
                                        add_one_height_with_transactions_from_other_validator};
    use configuration_service::{TxConfigPropose, TxConfigVote, ConfigurationService,
                                ConfigurationSchema};
    use serde_json::Value;
    use blockchain_explorer;

    pub fn configuration_sandbox() -> (Sandbox, SandboxState, StoredConfiguration) {
        let _ = blockchain_explorer::helpers::init_logger();
        let sandbox = sandbox_with_services(vec![Box::new(TimestampingService::new()),
                                                 Box::new(ConfigurationService::new())]);
        let sandbox_state = SandboxState::new();
        let initial_cfg = sandbox.cfg();
        (sandbox, sandbox_state, initial_cfg)
    }

    fn get_propose(sandbox: &Sandbox,
                   config_hash: Hash)
                   -> Result<Option<TxConfigPropose>, StorageError> {
        let view = sandbox.blockchain_ref().view();
        let schema = ConfigurationSchema::new(&view);
        schema.get_propose(&config_hash)
    }

    fn get_votes_for_propose(sandbox: &Sandbox,
                             config_hash: Hash)
                             -> Result<Vec<Option<TxConfigVote>>, StorageError> {
        let view = sandbox.blockchain_ref().view();
        let schema = ConfigurationSchema::new(&view);
        schema.get_votes(&config_hash)
    }

    #[test]
    fn test_full_node_to_validator() {
        use super::CfgStub;
        use exonum::blockchain::Service;
        use serde_json::Value;
        use serde_json::value::ToJson;
        let _ = blockchain_explorer::helpers::init_logger();
        let (sandbox, sandbox_state, initial_cfg) = configuration_sandbox();
        sandbox.assert_state(1, 1);
        add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
        sandbox.assert_state(2, 1);

        assert_eq!(sandbox.is_validator(), true);

        let validators = sandbox.validators();

        let mut services: BTreeMap<String, Value> = BTreeMap::new();
        let tmstmp_id = TimestampingService::new().service_id();
        let service_cfg = CfgStub { cfg_string: "some test".to_string() };
        services.insert(format!("{}", tmstmp_id), service_cfg.to_json());


        let full_node_cfg = StoredConfiguration {
            previous_cfg_hash: initial_cfg.hash(),
            actual_from: 4,
            validators: validators[1..].iter().cloned().collect(),
            consensus: sandbox.cfg().consensus,
            services: services.clone(),
        };



        {
            let propose_tx = TxConfigPropose::new(&sandbox.p(1),
                                                  &full_node_cfg.clone().serialize(),
                                                  sandbox.s(1));
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[propose_tx.raw().clone()]);
        }
        {
            let mut votes = Vec::new();
            for validator in 0..3 {
                votes.push(TxConfigVote::new(&sandbox.p(validator),
                                             &full_node_cfg.hash(),
                                             sandbox.s(validator))
                                   .raw()
                                   .clone());
            }
            add_one_height_with_transactions(&sandbox, &sandbox_state, &votes);
        }
        sandbox.assert_state(4, 1);
        assert_eq!(full_node_cfg, sandbox.cfg());
        assert_eq!(sandbox.is_validator(), false);

        let validator_cfg = StoredConfiguration {
            previous_cfg_hash: full_node_cfg.hash(),
            actual_from: 6,
            validators: validators[0..].iter().cloned().collect(),
            consensus: sandbox.cfg().consensus,
            services: services.clone(),
        };

        {
            let propose_tx = TxConfigPropose::new(&sandbox.p(1),
                                                  &validator_cfg.clone().serialize(),
                                                  sandbox.s(1));
            add_one_height_with_transactions_from_other_validator(&sandbox,
                                                                  &sandbox_state,
                                                                  &[propose_tx.raw().clone()]);
        }
        {
            let mut votes = Vec::new();
            for validator in 0..3 {
                votes.push(TxConfigVote::new(&sandbox.p(validator),
                                             &validator_cfg.hash(),
                                             sandbox.s(validator))
                                   .raw()
                                   .clone());
            }
            add_one_height_with_transactions_from_other_validator(&sandbox, &sandbox_state, &votes);
        }
        sandbox.assert_state(6, 1);
        assert_eq!(validator_cfg, sandbox.cfg());
        assert_eq!(sandbox.is_validator(), true);

    }

    #[test]
    fn test_add_validators_to_config() {
        let (mut sandbox, sandbox_state, initial_cfg) = configuration_sandbox();
        let start_time = sandbox.time();
        sandbox.assert_state(1, 1);

        let new_keypairs = (40..44)
            .map(|seed_num| gen_keypair_from_seed(&Seed::new([seed_num; SEED_LENGTH])))
            .collect::<Vec<_>>();
        let mut validators = sandbox.validators();
        let old_len = validators.len();
        validators.extend(new_keypairs.iter().map(|el| el.0));
        let new_len = validators.len();

        let actual_from = 3;
        let services: BTreeMap<String, Value> = BTreeMap::new();
        let added_keys_cfg = StoredConfiguration {
            previous_cfg_hash: initial_cfg.hash(),
            actual_from: actual_from,
            validators: validators.clone(),
            consensus: sandbox.cfg().consensus,
            services: services,
        };
        {
            let propose_tx = TxConfigPropose::new(&sandbox.p(1),
                                                  &added_keys_cfg.clone().serialize(),
                                                  sandbox.s(1));
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[propose_tx.raw().clone()]);
            sandbox.assert_state(2, 1);
        }
        {
            let mut votes = Vec::new();
            for validator in 0..3 {
                votes.push(TxConfigVote::new(&sandbox.p(validator),
                                             &added_keys_cfg.hash(),
                                             sandbox.s(validator))
                                   .raw()
                                   .clone());
            }
            add_one_height_with_transactions(&sandbox, &sandbox_state, &votes);
            sandbox.assert_state(3, 1);
        }
        {
            sandbox.set_validators_map(new_len as u8, new_keypairs);
            sandbox.initialize(start_time, old_len, new_len);
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
            sandbox.assert_state(4, 1);
        }
        let new_cfg = generate_config_with_message(added_keys_cfg.hash(), 8, "First cfg", &sandbox);
        {
            let propose_tx =
                TxConfigPropose::new(&sandbox.p(1), &new_cfg.clone().serialize(), sandbox.s(1));
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[propose_tx.raw().clone()]);
            sandbox.assert_state(5, 1);
        }
        {
            let mut votes = Vec::new();
            for validator in 0..5 {
                votes.push(TxConfigVote::new(&sandbox.p(validator),
                                             &new_cfg.hash(),
                                             sandbox.s(validator))
                                   .raw()
                                   .clone());
            }
            add_one_height_with_transactions(&sandbox, &sandbox_state, &votes);
            sandbox.assert_state(6, 1);
            assert_eq!(None, sandbox.following_cfg());
        }
        {
            let last_vote = TxConfigVote::new(&sandbox.p(5), &new_cfg.hash(), sandbox.s(5));
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[last_vote.raw().clone()]);
            sandbox.assert_state(7, 1);
            assert_eq!(Some(new_cfg), sandbox.following_cfg());
        }
    }

    #[test]
    fn test_exclude_sandbox_node_from_config() {
        let (sandbox, sandbox_state, initial_cfg) = configuration_sandbox();
        sandbox.assert_state(1, 1);

        let new_public_keys = (40..44)
            .map(|seed_num| gen_keypair_from_seed(&Seed::new([seed_num; SEED_LENGTH])).0)
            .collect::<Vec<_>>();

        let actual_from = 3;
        let services: BTreeMap<String, Value> = BTreeMap::new();
        let excluding_cfg = StoredConfiguration {
            previous_cfg_hash: initial_cfg.hash(),
            actual_from: actual_from,
            validators: new_public_keys,
            consensus: sandbox.cfg().consensus,
            services: services,
        };
        {
            let propose_tx = TxConfigPropose::new(&sandbox.p(1),
                                                  &excluding_cfg.clone().serialize(),
                                                  sandbox.s(1));
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[propose_tx.raw().clone()]);
            sandbox.assert_state(2, 1);
        }
        {
            let mut votes = Vec::new();
            for validator in 0..3 {
                votes.push(TxConfigVote::new(&sandbox.p(validator),
                                             &excluding_cfg.hash(),
                                             sandbox.s(validator))
                                   .raw()
                                   .clone());
            }
            add_one_height_with_transactions(&sandbox, &sandbox_state, &votes);
            sandbox.assert_state(3, 1);
        }
    }

    #[test]
    fn test_discard_propose_for_same_cfg() {
        let (sandbox, sandbox_state, initial_cfg) = configuration_sandbox();
        sandbox.assert_state(1, 1);
        let new_cfg = generate_config_with_message(initial_cfg.hash(), 4, "First cfg", &sandbox);
        let propose_tx =
            TxConfigPropose::new(&sandbox.p(1), &new_cfg.clone().serialize(), sandbox.s(1));
        {
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[propose_tx.raw().clone()]);
            sandbox.assert_state(2, 1);
            assert_eq!(Some(propose_tx.clone()),
                       get_propose(&sandbox, new_cfg.hash()).unwrap());
        }
        {
            let duplicate_cfg_propose =
                TxConfigPropose::new(&sandbox.p(0), &new_cfg.clone().serialize(), sandbox.s(0));
            add_one_height_with_transactions(&sandbox,
                                             &sandbox_state,
                                             &[duplicate_cfg_propose.raw().clone()]);
            sandbox.assert_state(3, 1);
            assert_eq!(Some(propose_tx),
                       get_propose(&sandbox, new_cfg.hash()).unwrap());
        }
    }

    #[test]
    fn test_discard_vote_for_absent_propose() {
        let (sandbox, sandbox_state, initial_cfg) = configuration_sandbox();
        sandbox.assert_state(1, 1);
        let new_cfg = generate_config_with_message(initial_cfg.hash(), 4, "First cfg", &sandbox);
        let absent_cfg =
            generate_config_with_message(initial_cfg.hash(), 4, "Absent propose", &sandbox);
        {
            let propose_tx =
                TxConfigPropose::new(&sandbox.p(1), &new_cfg.clone().serialize(), sandbox.s(1));
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[propose_tx.raw().clone()]);
            sandbox.assert_state(2, 1);
        }
        {
            let legal_vote = TxConfigVote::new(&sandbox.p(3), &new_cfg.hash(), sandbox.s(3));
            let illegal_vote = TxConfigVote::new(&sandbox.p(3), &absent_cfg.hash(), sandbox.s(3));
            add_one_height_with_transactions(&sandbox,
                                             &sandbox_state,
                                             &[legal_vote.raw().clone(),
                                               illegal_vote.raw().clone()]);
            sandbox.assert_state(3, 1);
            let votes = get_votes_for_propose(&sandbox, new_cfg.hash()).unwrap();
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
            sandbox.assert_state(target_height, 1);
        }
        let new_cfg =
            generate_config_with_message(initial_cfg.hash(), target_height, "First cfg", &sandbox);
        {
            let propose_tx =
                TxConfigPropose::new(&sandbox.p(1), &new_cfg.clone().serialize(), sandbox.s(1));
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[propose_tx.raw().clone()]);
            sandbox.assert_state(target_height + 1, 1);
            assert_eq!(None, get_propose(&sandbox, new_cfg.hash()).unwrap());
        }
    }

    #[test]
    fn test_discard_votes_with_expired_actual_from() {
        let (sandbox, sandbox_state, initial_cfg) = configuration_sandbox();
        sandbox.assert_state(1, 1);
        let target_height = 10;

        let new_cfg =
            generate_config_with_message(initial_cfg.hash(), target_height, "First cfg", &sandbox);
        {
            let propose_tx =
                TxConfigPropose::new(&sandbox.p(1), &new_cfg.clone().serialize(), sandbox.s(1));
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[propose_tx.raw().clone()]);
            sandbox.assert_state(2, 1);
            assert_eq!(Some(propose_tx),
                       get_propose(&sandbox, new_cfg.hash()).unwrap());
        }
        {
            let legal_vote = TxConfigVote::new(&sandbox.p(3), &new_cfg.hash(), sandbox.s(3));
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[legal_vote.raw().clone()]);
            sandbox.assert_state(3, 1);
            let votes = get_votes_for_propose(&sandbox, new_cfg.hash()).unwrap();
            assert!(votes.contains(&Some(legal_vote)));
        }
        {
            for _ in 3..target_height {
                add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
            }
            sandbox.assert_state(target_height, 1);
        }
        {
            let illegal_vote = TxConfigVote::new(&sandbox.p(0), &new_cfg.hash(), sandbox.s(0));
            add_one_height_with_transactions(&sandbox,
                                             &sandbox_state,
                                             &[illegal_vote.raw().clone()]);
            sandbox.assert_state(target_height + 1, 1);
            let votes = get_votes_for_propose(&sandbox, new_cfg.hash()).unwrap();
            assert!(!votes.contains(&Some(illegal_vote)));
        }
    }

    #[test]
    fn test_discard_invalid_config_json() {
        let (sandbox, sandbox_state, _) = configuration_sandbox();
        sandbox.assert_state(1, 1);
        let new_cfg = [70; 74]; //invalid json bytes
        {
            let propose_tx = TxConfigPropose::new(&sandbox.p(1), &new_cfg, sandbox.s(1));
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[propose_tx.raw().clone()]);
            sandbox.assert_state(2, 1);
            assert_eq!(None, get_propose(&sandbox, hash(&new_cfg)).unwrap());
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
            sandbox.assert_state(target_height + 1, 1);
        }
        let new_cfg = generate_config_with_message(initial_cfg.hash(),
                                                   target_height + 4,
                                                   "First cfg",
                                                   &sandbox);
        {
            let propose_tx =
                TxConfigPropose::new(&sandbox.p(1), &new_cfg.clone().serialize(), sandbox.s(1));
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[propose_tx.raw().clone()]);
            sandbox.assert_state(target_height + 2, 1);
            assert_eq!(propose_tx,
                       get_propose(&sandbox, new_cfg.hash()).unwrap().unwrap());
        }
        {
            let mut expected_votes = Vec::new();
            for validator in 0..2 {
                expected_votes.push(TxConfigVote::new(&sandbox.p(validator),
                                                      &new_cfg.hash(),
                                                      sandbox.s(validator))
                                            .raw()
                                            .clone());
            }
            let unposted_vote = TxConfigVote::new(&sandbox.p(2), &new_cfg.hash(), sandbox.s(2));
            add_one_height_with_transactions(&sandbox, &sandbox_state, &expected_votes);
            sandbox.assert_state(target_height + 3, 1);
            let actual_votes = get_votes_for_propose(&sandbox, new_cfg.hash()).unwrap();
            for raw_vote in expected_votes {
                let exp_vote = TxConfigVote::from_raw(raw_vote).unwrap();
                assert!(actual_votes.contains(&Some(exp_vote)));
            }
            assert!(!actual_votes.contains(&Some(unposted_vote)));
            assert_eq!(initial_cfg, sandbox.cfg());
            assert_eq!(None, sandbox.following_cfg());
        }
        {
            let vote3 = TxConfigVote::new(&sandbox.p(2), &new_cfg.hash(), sandbox.s(2));
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[vote3.raw().clone()]);
            sandbox.assert_state(target_height + 4, 1);
            let votes = get_votes_for_propose(&sandbox, new_cfg.hash()).unwrap();
            assert!(votes.contains(&Some(vote3)));
            assert_eq!(new_cfg, sandbox.cfg());
        }
    }

    #[test]
    fn test_config_txs_discarded_when_following_config_present() {
        let (sandbox, sandbox_state, initial_cfg) = configuration_sandbox();
        sandbox.assert_state(1, 1);

        let following_config = generate_config_with_message(initial_cfg.hash(),
                                                            6,
                                                            "Following cfg at height 6",
                                                            &sandbox);

        {
            let propose_tx = TxConfigPropose::new(&sandbox.p(1),
                                                  &following_config.clone().serialize(),
                                                  sandbox.s(1));
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[propose_tx.raw().clone()]);
            sandbox.assert_state(2, 1);
            assert_eq!(Some(propose_tx),
                       get_propose(&sandbox, following_config.hash()).unwrap());
        }
        {
            let votes = (0..3)
                .map(|validator| {
                         TxConfigVote::new(&sandbox.p(validator),
                                           &following_config.hash(),
                                           sandbox.s(validator))
                                 .raw()
                                 .clone()
                     })
                .collect::<Vec<_>>();
            add_one_height_with_transactions(&sandbox, &sandbox_state, &votes);
            sandbox.assert_state(3, 1);
            assert_eq!(sandbox.cfg(), initial_cfg);
            assert_eq!(sandbox.following_cfg(), Some(following_config.clone()));
        }
        let new_cfg = generate_config_with_message(initial_cfg.hash(), 7, "New cfg", &sandbox);

        {
            let propose_tx_new =
                TxConfigPropose::new(&sandbox.p(1), &new_cfg.clone().serialize(), sandbox.s(1));
            add_one_height_with_transactions(&sandbox,
                                             &sandbox_state,
                                             &[propose_tx_new.raw().clone()]);
            sandbox.assert_state(4, 1);

            assert_eq!(None, get_propose(&sandbox, new_cfg.hash()).unwrap());
        }
        let vote_validator_0 =
            TxConfigVote::new(&sandbox.p(0), &following_config.hash(), sandbox.s(0));
        let vote_validator_3 =
            TxConfigVote::new(&sandbox.p(3), &following_config.hash(), sandbox.s(3));
        {
            add_one_height_with_transactions(&sandbox,
                                             &sandbox_state,
                                             &[vote_validator_3.raw().clone()]);
            sandbox.assert_state(5, 1);

            let votes = get_votes_for_propose(&sandbox, following_config.hash()).unwrap();
            assert!(votes.contains(&Some(vote_validator_0)));
            assert!(!votes.contains(&Some(vote_validator_3)));
            assert_eq!(initial_cfg, sandbox.cfg());
        }
        {
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
            sandbox.assert_state(6, 1);
            assert_eq!(following_config, sandbox.cfg());
        }
    }

    #[test]
    fn test_config_txs_discarded_when_not_referencing_actual_config_or_sent_by_illegal_validator
        () {
        let (sandbox, sandbox_state, initial_cfg) = configuration_sandbox();
        sandbox.assert_state(1, 1);

        let new_cfg_bad_previous_cfg = generate_config_with_message(Hash::new([11; HASH_SIZE]),
                                                                    6,
                                                                    "Following cfg at height 6",
                                                                    &sandbox);
        // not actual config hash

        let new_cfg = generate_config_with_message(initial_cfg.hash(),
                                                   6,
                                                   "Following cfg at height 6",
                                                   &sandbox);
        let new_cfg_discarded_votes =
            generate_config_with_message(initial_cfg.hash(), 8, "discarded votes", &sandbox);

        let (illegal_pub, illegal_sec) = gen_keypair_from_seed(&Seed::new([66; 32]));

        {
            let illegal_propose1 =
                TxConfigPropose::new(&sandbox.p(1),
                                     &new_cfg_bad_previous_cfg.clone().serialize(),
                                     sandbox.s(1));
            let illegal_propose2 = TxConfigPropose::new(&illegal_pub,
                                                        // not a member of actual config
                                                        &new_cfg.clone().serialize(),
                                                        &illegal_sec);
            add_one_height_with_transactions(&sandbox,
                                             &sandbox_state,
                                             &[illegal_propose1.raw().clone(),
                                               illegal_propose2.raw().clone()]);
            sandbox.assert_state(2, 1);
            assert_eq!(None,
                       get_propose(&sandbox, new_cfg_bad_previous_cfg.hash()).unwrap());
            assert_eq!(None, get_propose(&sandbox, new_cfg.hash()).unwrap());
        }
        {
            let legal_propose1 =
                TxConfigPropose::new(&sandbox.p(1), &new_cfg.clone().serialize(), sandbox.s(1));
            let legal_propose2 = TxConfigPropose::new(&sandbox.p(1),
                                                      &new_cfg_discarded_votes.clone().serialize(),
                                                      sandbox.s(1));
            add_one_height_with_transactions(&sandbox,
                                             &sandbox_state,
                                             &[legal_propose1.raw().clone(),
                                               legal_propose2.raw().clone()]);
            sandbox.assert_state(3, 1);
            assert_eq!(Some(legal_propose1),
                       get_propose(&sandbox, new_cfg.hash()).unwrap());
            assert_eq!(Some(legal_propose2),
                       get_propose(&sandbox, new_cfg_discarded_votes.hash()).unwrap());
        }
        {
            let illegal_validator_vote =
                TxConfigVote::new(&illegal_pub, &new_cfg_discarded_votes.hash(), &illegal_sec);
            add_one_height_with_transactions(&sandbox,
                                             &sandbox_state,
                                             &[illegal_validator_vote.raw().clone()]);
            sandbox.assert_state(4, 1);
            let votes = get_votes_for_propose(&sandbox, new_cfg_discarded_votes.hash()).unwrap();
            assert!(!votes.contains(&Some(illegal_validator_vote)));
        }
        {
            let votes = (0..3)
                .map(|validator| {
                         TxConfigVote::new(&sandbox.p(validator),
                                           &new_cfg.hash(),
                                           sandbox.s(validator))
                                 .raw()
                                 .clone()
                     })
                .collect::<Vec<_>>();
            add_one_height_with_transactions(&sandbox, &sandbox_state, &votes);
            sandbox.assert_state(5, 1);
            assert_eq!(initial_cfg, sandbox.cfg());
            assert_eq!(Some(new_cfg.clone()), sandbox.following_cfg());
        }
        {
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
            sandbox.assert_state(6, 1);
            assert_eq!(new_cfg, sandbox.cfg());
            assert_eq!(None, sandbox.following_cfg());
        }
        {
            let expected_votes = (0..3)
                .map(|validator| {
                         TxConfigVote::new(&sandbox.p(validator),
                                           &new_cfg_discarded_votes.hash(),
                                           sandbox.s(validator))
                                 .raw()
                                 .clone()
                     })
                .collect::<Vec<_>>();
            add_one_height_with_transactions(&sandbox, &sandbox_state, &expected_votes);
            sandbox.assert_state(7, 1);
            let actual_votes = get_votes_for_propose(&sandbox, new_cfg_discarded_votes.hash())
                .unwrap();
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
        sandbox.assert_state(1, 1);

        let actual_from = 5;

        let new_cfg1 =
            generate_config_with_message(initial_cfg.hash(), actual_from, "First cfg", &sandbox);
        let new_cfg2 =
            generate_config_with_message(initial_cfg.hash(), actual_from, "Second cfg", &sandbox);
        {
            let mut proposes = Vec::new();
            for cfg in &[new_cfg1.clone(), new_cfg2.clone()] {
                proposes.push(TxConfigPropose::new(&sandbox.p(1),
                                                   &cfg.clone().serialize(),
                                                   sandbox.s(1))
                                      .raw()
                                      .clone());
            }

            add_one_height_with_transactions(&sandbox, &sandbox_state, &proposes);
            sandbox.assert_state(2, 1);
        }
        {
            let mut votes = Vec::new();
            for validator in 0..2 {
                votes.push(TxConfigVote::new(&sandbox.p(validator),
                                             &new_cfg1.hash(),
                                             sandbox.s(validator))
                                   .raw()
                                   .clone());
            }

            add_one_height_with_transactions(&sandbox, &sandbox_state, &votes);
            sandbox.assert_state(3, 1);
            assert_eq!(initial_cfg, sandbox.cfg());
        }
        {
            let prop2_validator2 = TxConfigVote::new(&sandbox.p(2), &new_cfg2.hash(), sandbox.s(2));

            add_one_height_with_transactions(&sandbox,
                                             &sandbox_state,
                                             &[prop2_validator2.raw().clone()]);
            sandbox.assert_state(4, 1);
            assert_eq!(initial_cfg, sandbox.cfg());
        }
        {
            let prop1_validator2 = TxConfigVote::new(&sandbox.p(2), &new_cfg1.hash(), sandbox.s(2));

            add_one_height_with_transactions(&sandbox,
                                             &sandbox_state,
                                             &[prop1_validator2.raw().clone()]);
            sandbox.assert_state(5, 1);
            assert_eq!(new_cfg1, sandbox.cfg());
        }
    }

    #[test]
    fn test_regression_new_vote_for_older_config_applies_old_config() {
        let (sandbox, sandbox_state, initial_cfg) = configuration_sandbox();
        sandbox.assert_state(1, 1);

        let new_cfg1 = generate_config_with_message(initial_cfg.hash(), 3, "First cfg", &sandbox);
        let new_cfg2 = generate_config_with_message(new_cfg1.hash(), 5, "Second cfg", &sandbox);

        {
            let propose_tx1 =
                TxConfigPropose::new(&sandbox.p(1), &new_cfg1.clone().serialize(), sandbox.s(1));

            add_one_height_with_transactions(&sandbox,
                                             &sandbox_state,
                                             &[propose_tx1.raw().clone()]);
            sandbox.assert_state(2, 1);
        }
        {
            let mut votes_for_new_cfg1 = Vec::new();
            for validator in 0..3 {
                votes_for_new_cfg1.push(TxConfigVote::new(&sandbox.p(validator),
                                                          &new_cfg1.hash(),
                                                          sandbox.s(validator))
                                                .raw()
                                                .clone());
            }
            add_one_height_with_transactions(&sandbox, &sandbox_state, &votes_for_new_cfg1);
            sandbox.assert_state(3, 1);
            assert_eq!(new_cfg1, sandbox.cfg());
        }
        {
            let propose_tx2 =
                TxConfigPropose::new(&sandbox.p(1), &new_cfg2.clone().serialize(), sandbox.s(1));

            add_one_height_with_transactions(&sandbox,
                                             &sandbox_state,
                                             &[propose_tx2.raw().clone()]);
            sandbox.assert_state(4, 1);
        }
        {
            let mut votes_for_new_cfg2 = Vec::new();
            for validator in 0..3 {
                votes_for_new_cfg2.push(TxConfigVote::new(&sandbox.p(validator),
                                                          &new_cfg2.hash(),
                                                          sandbox.s(validator))
                                                .raw()
                                                .clone());
            }
            add_one_height_with_transactions(&sandbox, &sandbox_state, &votes_for_new_cfg2);
            sandbox.assert_state(5, 1);
            assert_eq!(new_cfg2, sandbox.cfg());
        }
        {
            let prop1_validator3 = TxConfigVote::new(&sandbox.p(3), &new_cfg1.hash(), sandbox.s(3));
            add_one_height_with_transactions(&sandbox,
                                             &sandbox_state,
                                             &[prop1_validator3.raw().clone()]);
            sandbox.assert_state(6, 1);
            assert_eq!(new_cfg2, sandbox.cfg());
        }
    }
}
