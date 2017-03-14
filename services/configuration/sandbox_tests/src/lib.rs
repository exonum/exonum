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
    use exonum::storage::Map;
    use exonum::messages::Message;
    use exonum::blockchain::config::StoredConfiguration;
    use exonum::blockchain::Service;
    use sandbox::sandbox_with_services;
    use sandbox::sandbox::Sandbox;
    use sandbox::timestamping::TimestampingService;
    use sandbox::sandbox_tests_helper::{SandboxState, add_one_height_with_transactions};
    use configuration_service::{TxConfigPropose, TxConfigVote, ConfigurationService,
                                ConfigurationSchema};
    use serde_json::{to_vec, Value};
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
    fn test_change_service_config() {
        let _ = blockchain_explorer::helpers::init_logger();
        let sandbox = configuration_sandbox();
        let sandbox_state = SandboxState::new();
        let initial_cfg = sandbox.cfg();

        add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
        add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
        add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
        add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
        sandbox.assert_state(5, 1);

        let services = gen_timestamping_cfg("First cfg");

        let new_cfg = StoredConfiguration {
            actual_from: 7,
            validators: sandbox.validators(),
            consensus: sandbox.cfg().consensus,
            services: services,
        };
        // let ser_conf = to_vec(new_cfg).unwrap();
        let propose_tx = TxConfigPropose::new(&sandbox.p(1),
                                              5,
                                              &to_vec(&new_cfg).unwrap(),
                                              6,
                                              sandbox.s(1));
        add_one_height_with_transactions(&sandbox, &sandbox_state, &[propose_tx.raw().clone()]);
        sandbox.assert_state(6, 1);
        let view = sandbox.blockchain_copy().view();
        let schema = ConfigurationSchema::new(&view);
        let proposes = schema.config_proposes();
        let propose_hash = propose_tx.hash();

        assert_eq!(propose_tx, proposes.get(&propose_hash).unwrap().unwrap());
        let vote1 = TxConfigVote::new(&sandbox.p(0), 5, &propose_hash, 1700, false, sandbox.s(0));
        let vote2 = TxConfigVote::new(&sandbox.p(1), 5, &propose_hash, 1700, false, sandbox.s(1));

        add_one_height_with_transactions(&sandbox,
                                         &sandbox_state,
                                         &[vote1.raw().clone(), vote2.raw().clone()]);
        sandbox.assert_state(7, 1);
        let view = sandbox.blockchain_copy().view();
        let schema = ConfigurationSchema::new(&view);
        let votes = schema.config_votes();
        assert_eq!(vote1, votes.get(&sandbox.p(0)).unwrap().unwrap());
        assert_eq!(vote2, votes.get(&sandbox.p(1)).unwrap().unwrap());
        assert_eq!(None, votes.get(&sandbox.p(2)).unwrap());
        assert_eq!(initial_cfg, sandbox.cfg());

        let vote3 = TxConfigVote::new(&sandbox.p(2), 5, &propose_hash, 1700, false, sandbox.s(2));
        add_one_height_with_transactions(&sandbox, &sandbox_state, &[vote3.raw().clone()]);
        let view = sandbox.blockchain_copy().view();
        let schema = ConfigurationSchema::new(&view);
        let votes = schema.config_votes();
        sandbox.assert_state(8, 1);
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
        add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
        sandbox.assert_state(2, 1);

        let actual_from = 4;

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

        let propose_tx1 = TxConfigPropose::new(&sandbox.p(1),
                                               5,
                                               &to_vec(&new_cfg1).unwrap(),
                                               actual_from,
                                               sandbox.s(1));
        let propose_tx2 = TxConfigPropose::new(&sandbox.p(1),
                                               5,
                                               &to_vec(&new_cfg2).unwrap(),
                                               actual_from,
                                               sandbox.s(1));

        add_one_height_with_transactions(&sandbox,
                                         &sandbox_state,
                                         &[propose_tx1.raw().clone(), propose_tx2.raw().clone()]);
        sandbox.assert_state(3, 1);

        let pr_hash1 = propose_tx1.hash();
        let pr_hash2 = propose_tx2.hash();
        let prop1_validator0 = TxConfigVote::new(&sandbox.p(0), 5, &pr_hash1, 1700, false, sandbox.s(0));
        let prop1_validator1 = TxConfigVote::new(&sandbox.p(1), 5, &pr_hash1, 1700, false, sandbox.s(1));

        add_one_height_with_transactions(&sandbox,
                                         &sandbox_state,
                                         &[prop1_validator0.raw().clone(), prop1_validator1.raw().clone()]);
        sandbox.assert_state(4, 1);
        assert_eq!(initial_cfg, sandbox.cfg());

        let prop2_validator2 = TxConfigVote::new(&sandbox.p(2), 5, &pr_hash2, 1700, false, sandbox.s(2));

        add_one_height_with_transactions(&sandbox,
                                         &sandbox_state,
                                         &[prop2_validator2.raw().clone()]);
        sandbox.assert_state(5, 1);
        assert_eq!(initial_cfg, sandbox.cfg());
    }

    #[test]
    fn test_regression_new_vote_for_older_config_applies_old_config() {

        let _ = blockchain_explorer::helpers::init_logger();
        let sandbox = configuration_sandbox();
        let sandbox_state = SandboxState::new();

        add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
        sandbox.assert_state(2, 1);

        let actual_from = 4;

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
        let propose_tx1 = TxConfigPropose::new(&sandbox.p(1),
                                               5,
                                               &to_vec(&new_cfg1).unwrap(),
                                               actual_from,
                                               sandbox.s(1));
        let propose_tx2 = TxConfigPropose::new(&sandbox.p(1),
                                               5,
                                               &to_vec(&new_cfg2).unwrap(),
                                               actual_from,
                                               sandbox.s(1));

        add_one_height_with_transactions(&sandbox,
                                         &sandbox_state,
                                         &[propose_tx1.raw().clone(), propose_tx2.raw().clone()]);
        sandbox.assert_state(3, 1);

        let pr_hash1 = propose_tx1.hash();
        let pr_hash2 = propose_tx2.hash();
        let prop1_validator0 = TxConfigVote::new(&sandbox.p(0), 5, &pr_hash1, 1700, false, sandbox.s(0));
        let prop1_validator1 = TxConfigVote::new(&sandbox.p(1), 5, &pr_hash1, 1700, false, sandbox.s(1));
        let prop1_validator2 = TxConfigVote::new(&sandbox.p(2), 5, &pr_hash1, 1700, false, sandbox.s(2));

        add_one_height_with_transactions(&sandbox,
                                         &sandbox_state,
                                         &[prop1_validator0.raw().clone(), prop1_validator1.raw().clone(), prop1_validator2.raw().clone()]);
        sandbox.assert_state(4, 1);
        assert_eq!(new_cfg1, sandbox.cfg());

        let prop2_validator0 = TxConfigVote::new(&sandbox.p(0), 5, &pr_hash2, 1701, false, sandbox.s(0));
        let prop2_validator1 = TxConfigVote::new(&sandbox.p(1), 5, &pr_hash2, 1701, false, sandbox.s(1));
        let prop2_validator2 = TxConfigVote::new(&sandbox.p(2), 5, &pr_hash2, 1701, false, sandbox.s(2));

        add_one_height_with_transactions(&sandbox,
                                         &sandbox_state,
                                         &[prop2_validator0.raw().clone(), prop2_validator1.raw().clone(), prop2_validator2.raw().clone()]);
        sandbox.assert_state(5, 1);
        assert_eq!(new_cfg2, sandbox.cfg());

        let prop1_validator0 = TxConfigVote::new(&sandbox.p(0), 5, &pr_hash1, 1702, false, sandbox.s(0));
        add_one_height_with_transactions(&sandbox,
                                         &sandbox_state,
                                         &[prop1_validator0.raw().clone()]);
        sandbox.assert_state(6, 1);
        assert_eq!(new_cfg2, sandbox.cfg());
    }
}
