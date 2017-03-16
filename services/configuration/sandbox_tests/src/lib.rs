#[macro_use]
extern crate serde_derive;
extern crate blockchain_explorer;
extern crate sandbox;
extern crate exonum;
extern crate configuration_service;
extern crate serde_json;
extern crate serde;
#[macro_use]
extern crate log;

#[cfg(test)]
mod tests {

    use std::collections::BTreeMap;
    use exonum::storage::{Map, StorageValue};
    use exonum::messages::Message;
    use exonum::blockchain::config::StoredConfiguration;
    use exonum::blockchain::Service;
    use sandbox::sandbox_with_services;
    use sandbox::sandbox::Sandbox;
    use sandbox::timestamping::TimestampingService;
    use sandbox::sandbox_tests_helper::{SandboxState, add_one_height_with_transactions};
    use configuration_service::{TxConfigPropose, TxConfigVote, ConfigurationService,
                                ConfigurationSchema};
    use serde_json::{Value};
    use serde_json::value::ToJson;
    use blockchain_explorer;


    pub fn configuration_sandbox() -> Sandbox {
        let sandbox = sandbox_with_services(vec![Box::new(TimestampingService::new()),
                                                 Box::new(ConfigurationService::new())]);
        sandbox
    }

    #[derive(Serialize)]
    struct CfgStub {
        cfg_string: String,
    }

    #[test]
    fn test_config_txs_discarded_when_following_config_present() {
        let _ = blockchain_explorer::helpers::init_logger();
        let sandbox = configuration_sandbox();
        let sandbox_state = SandboxState::new();
        let initial_cfg = sandbox.cfg();
        sandbox.assert_state(1, 1);

        let services = gen_timestamping_cfg("Following cfg at height 6");
        let following_config = StoredConfiguration {
            actual_from: 6,
            validators: sandbox.validators(),
            consensus: sandbox.cfg().consensus,
            services: services,
        };

        let foll_cfg_bytes = following_config.clone().serialize();
        let foll_cfg_hash = following_config.hash();
        let initial_cfg_hash = initial_cfg.hash();

        let propose_tx = TxConfigPropose::new(&sandbox.p(1),
                                              &initial_cfg_hash,
                                              &foll_cfg_bytes,
                                              sandbox.s(1));
        add_one_height_with_transactions(&sandbox, &sandbox_state, &[propose_tx.raw().clone()]);
        sandbox.assert_state(2, 1);
        let view = sandbox.blockchain_ref().view();
        let schema = ConfigurationSchema::new(&view);
        let proposes = schema.config_proposes();
        assert_eq!(Some(propose_tx), proposes.get(&foll_cfg_hash).unwrap());

        let votes = (0..3).map(|validator|{
            TxConfigVote::new(&sandbox.p(validator), &foll_cfg_hash, sandbox.s(validator)).raw().clone()
        }).collect::<Vec<_>>();
        add_one_height_with_transactions(&sandbox, &sandbox_state, &votes);
        sandbox.assert_state(3, 1);
        assert_eq!(sandbox.cfg(), initial_cfg);
        assert_eq!(sandbox.following_cfg(), Some(following_config.clone()));

        let services = gen_timestamping_cfg("New cfg");
        let new_cfg = StoredConfiguration {
            actual_from: 7,
            validators: sandbox.validators(),
            consensus: sandbox.cfg().consensus,
            services: services,
        };
        let new_cfg_bytes = new_cfg.clone().serialize();
        let new_cfg_hash = new_cfg.hash();

        let propose_tx_new = TxConfigPropose::new(&sandbox.p(1),
                                              &initial_cfg_hash,
                                              &new_cfg_bytes,
                                              sandbox.s(1));
        add_one_height_with_transactions(&sandbox, &sandbox_state, &[propose_tx_new.raw().clone()]);
        sandbox.assert_state(4, 1);

        let view = sandbox.blockchain_ref().view();
        let schema = ConfigurationSchema::new(&view);
        let proposes = schema.config_proposes();
        assert_eq!(None, proposes.get(&new_cfg_hash).unwrap());

        let vote_validator_3 = TxConfigVote::new(&sandbox.p(3), &foll_cfg_hash, sandbox.s(3));
        add_one_height_with_transactions(&sandbox, &sandbox_state, &[vote_validator_3.raw().clone()]);
        sandbox.assert_state(5, 1);

        let view = sandbox.blockchain_ref().view();
        let schema = ConfigurationSchema::new(&view);
        let votes_for_following_cfg = schema.config_votes(foll_cfg_hash);
        assert!(votes_for_following_cfg.get(&sandbox.p(0)).unwrap().is_some());
        assert!(votes_for_following_cfg.get(&sandbox.p(3)).unwrap().is_none());
        assert_eq!(initial_cfg, sandbox.cfg());

        add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
        sandbox.assert_state(6, 1);
        assert_eq!(following_config, sandbox.cfg());
    }

    #[test]
    fn test_change_service_config() {
        let _ = blockchain_explorer::helpers::init_logger();
        let sandbox = configuration_sandbox();
        let sandbox_state = SandboxState::new();
        let initial_cfg = sandbox.cfg();

        let target_height = 10;
        for _ in 0..target_height {
            add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
        }
        sandbox.assert_state(target_height + 1, 1);

        let services = gen_timestamping_cfg("First cfg");

        let new_cfg = StoredConfiguration {
            actual_from: target_height + 4,
            validators: sandbox.validators(),
            consensus: sandbox.cfg().consensus,
            services: services,
        };
        let new_cfg_bytes = new_cfg.clone().serialize();
        let new_cfg_hash = new_cfg.hash();
        let initial_cfg_hash = initial_cfg.hash();

        let propose_tx = TxConfigPropose::new(&sandbox.p(1),
                                              &initial_cfg_hash,
                                              &new_cfg_bytes,
                                              sandbox.s(1));
        add_one_height_with_transactions(&sandbox, &sandbox_state, &[propose_tx.raw().clone()]);
        sandbox.assert_state(target_height + 2, 1);

        let view = sandbox.blockchain_ref().view();
        let schema = ConfigurationSchema::new(&view);
        let proposes = schema.config_proposes();
        assert_eq!(propose_tx, proposes.get(&new_cfg_hash).unwrap().unwrap());

        let vote1 = TxConfigVote::new(&sandbox.p(0), &new_cfg_hash, sandbox.s(0));
        let vote2 = TxConfigVote::new(&sandbox.p(1), &new_cfg_hash, sandbox.s(1));
        add_one_height_with_transactions(&sandbox,
                                         &sandbox_state,
                                         &[vote1.raw().clone(), vote2.raw().clone()]);
        sandbox.assert_state(target_height + 3, 1);

        let view = sandbox.blockchain_ref().view();
        let schema = ConfigurationSchema::new(&view);
        let votes = schema.config_votes(new_cfg_hash);
        assert_eq!(vote1, votes.get(&sandbox.p(0)).unwrap().unwrap());
        assert_eq!(vote2, votes.get(&sandbox.p(1)).unwrap().unwrap());
        assert_eq!(None, votes.get(&sandbox.p(2)).unwrap());
        assert_eq!(initial_cfg, sandbox.cfg());

        let vote3 = TxConfigVote::new(&sandbox.p(2), &new_cfg_hash, sandbox.s(2));
        add_one_height_with_transactions(&sandbox, &sandbox_state, &[vote3.raw().clone()]);
        sandbox.assert_state(target_height + 4, 1);

        let view = sandbox.blockchain_ref().view();
        let schema = ConfigurationSchema::new(&view);
        let votes = schema.config_votes(new_cfg_hash);
        assert_eq!(vote3, votes.get(&sandbox.p(2)).unwrap().unwrap());
        assert_eq!(new_cfg, sandbox.cfg());
    }

    fn gen_timestamping_cfg(cfg_message: &str) -> BTreeMap<u16, Value> {
        let mut services: BTreeMap<u16, Value> = BTreeMap::new();
        let tmstmp_id = TimestampingService::new().service_id();
        let service_cfg = CfgStub { cfg_string: cfg_message.to_string() };
        services.insert(tmstmp_id, service_cfg.to_json());
        services
    }

    /// regression: votes' were summed for all proposes simultaneously, and not for the same propose
    #[test]
    fn test_regression_majority_votes_for_different_proposes() {

        let _ = blockchain_explorer::helpers::init_logger();
        let sandbox = configuration_sandbox();
        let sandbox_state = SandboxState::new();

        let initial_cfg = sandbox.cfg();
        let initial_cfg_hash = initial_cfg.hash();
        add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
        sandbox.assert_state(2, 1);

        let actual_from = 6;

        let services = gen_timestamping_cfg("First cfg");
        let new_cfg1 = StoredConfiguration {
            actual_from: actual_from,
            validators: sandbox.validators(),
            consensus: sandbox.cfg().consensus,
            services: services,
        };
        let services = gen_timestamping_cfg("Second cfg");
        let new_cfg2 = StoredConfiguration {
            actual_from: actual_from,
            validators: sandbox.validators(),
            consensus: sandbox.cfg().consensus,
            services: services,
        };
        let new_cfg1_bytes = new_cfg1.clone().serialize();
        let new_cfg2_bytes = new_cfg2.clone().serialize();
        let new_cfg1_hash =  new_cfg1.hash();
        let new_cfg2_hash =  new_cfg2.hash();


        let propose_tx1 = TxConfigPropose::new(&sandbox.p(1),
                                               &initial_cfg_hash,
                                               &new_cfg1_bytes,
                                               sandbox.s(1));
        let propose_tx2 = TxConfigPropose::new(&sandbox.p(1),
                                               &initial_cfg_hash,
                                               &new_cfg2_bytes,
                                               sandbox.s(1));

        add_one_height_with_transactions(&sandbox,
                                         &sandbox_state,
                                         &[propose_tx1.raw().clone(), propose_tx2.raw().clone()]);
        sandbox.assert_state(3, 1);

        let prop1_validator0 =
            TxConfigVote::new(&sandbox.p(0), &new_cfg1_hash, sandbox.s(0));
        let prop1_validator1 =
            TxConfigVote::new(&sandbox.p(1), &new_cfg1_hash, sandbox.s(1));

        add_one_height_with_transactions(&sandbox,
                                         &sandbox_state,
                                         &[prop1_validator0.raw().clone(),
                                           prop1_validator1.raw().clone()]);
        sandbox.assert_state(4, 1);
        assert_eq!(initial_cfg, sandbox.cfg());

        let prop2_validator2 =
            TxConfigVote::new(&sandbox.p(2), &new_cfg2_hash, sandbox.s(2));

        add_one_height_with_transactions(&sandbox,
                                         &sandbox_state,
                                         &[prop2_validator2.raw().clone()]);
        sandbox.assert_state(5, 1);
        assert_eq!(initial_cfg, sandbox.cfg());

        let prop1_validator2 =
            TxConfigVote::new(&sandbox.p(2), &new_cfg1_hash, sandbox.s(2));

        add_one_height_with_transactions(&sandbox,
                                         &sandbox_state,
                                         &[prop1_validator2.raw().clone()]);
        sandbox.assert_state(6, 1);
        assert_eq!(new_cfg1, sandbox.cfg());
    }

    #[test]
    fn test_regression_new_vote_for_older_config_applies_old_config() {

        let _ = blockchain_explorer::helpers::init_logger();
        let sandbox = configuration_sandbox();
        let sandbox_state = SandboxState::new();

        let initial_cfg = sandbox.cfg();
        let initial_cfg_hash = initial_cfg.hash();

        add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
        sandbox.assert_state(2, 1);

        let services = gen_timestamping_cfg("First cfg");
        let new_cfg1 = StoredConfiguration {
            actual_from: 4,
            validators: sandbox.validators(),
            consensus: sandbox.cfg().consensus,
            services: services,
        };
        let services = gen_timestamping_cfg("Second cfg");
        let new_cfg2 = StoredConfiguration {
            actual_from: 6,
            validators: sandbox.validators(),
            consensus: sandbox.cfg().consensus,
            services: services,
        };
        let new_cfg1_bytes = new_cfg1.clone().serialize();
        let new_cfg2_bytes = new_cfg2.clone().serialize();
        let new_cfg1_hash =  new_cfg1.hash();
        let new_cfg2_hash =  new_cfg2.hash();


        let propose_tx1 = TxConfigPropose::new(&sandbox.p(1),
                                               &initial_cfg_hash,
                                               &new_cfg1_bytes,
                                               sandbox.s(1));

        add_one_height_with_transactions(&sandbox,
                                         &sandbox_state,
                                         &[propose_tx1.raw().clone()]);
        sandbox.assert_state(3, 1);

        let prop1_validator0 =
            TxConfigVote::new(&sandbox.p(0), &new_cfg1_hash, sandbox.s(0));
        let prop1_validator1 =
            TxConfigVote::new(&sandbox.p(1), &new_cfg1_hash, sandbox.s(1));
        let prop1_validator2 =
            TxConfigVote::new(&sandbox.p(2), &new_cfg1_hash, sandbox.s(2));

        add_one_height_with_transactions(&sandbox,
                                         &sandbox_state,
                                         &[prop1_validator0.raw().clone(),
                                           prop1_validator1.raw().clone(),
                                           prop1_validator2.raw().clone()]);
        sandbox.assert_state(4, 1);
        assert_eq!(new_cfg1, sandbox.cfg());

        let propose_tx2 = TxConfigPropose::new(&sandbox.p(1),
                                               &new_cfg1_hash,
                                               &new_cfg2_bytes,
                                               sandbox.s(1));

        add_one_height_with_transactions(&sandbox,
                                         &sandbox_state,
                                         &[propose_tx2.raw().clone()]);
        sandbox.assert_state(5, 1);

        let prop2_validator0 =
            TxConfigVote::new(&sandbox.p(0), &new_cfg2_hash, sandbox.s(0));
        let prop2_validator1 =
            TxConfigVote::new(&sandbox.p(1), &new_cfg2_hash, sandbox.s(1));
        let prop2_validator2 =
            TxConfigVote::new(&sandbox.p(2), &new_cfg2_hash, sandbox.s(2));

        add_one_height_with_transactions(&sandbox,
                                         &sandbox_state,
                                         &[prop2_validator0.raw().clone(),
                                           prop2_validator1.raw().clone(),
                                           prop2_validator2.raw().clone()]);
        sandbox.assert_state(6, 1);
        assert_eq!(new_cfg2, sandbox.cfg());

        let prop1_validator3 =
            TxConfigVote::new(&sandbox.p(3), &new_cfg1_hash, sandbox.s(3));
        add_one_height_with_transactions(&sandbox,
                                         &sandbox_state,
                                         &[prop1_validator3.raw().clone()]);
        sandbox.assert_state(7, 1);
        assert_eq!(new_cfg2, sandbox.cfg());
    }
}
