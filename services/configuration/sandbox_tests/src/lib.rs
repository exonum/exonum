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

    #[test]
    fn it_works() {
        blockchain_explorer::helpers::init_logger();
        let sandbox = configuration_sandbox();
        let sandbox_state = SandboxState::new();

        add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
        add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
        add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
        add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
        sandbox.assert_state(5, 1);

        #[derive(Serialize)]
        struct CfgStub {
            cfg_string: String,
        }

        let mut services: BTreeMap<u16, Value> = BTreeMap::new();
        let tmstmp_id = TimestampingService::new().service_id();
        let service_cfg = CfgStub { cfg_string: "First_config_change".to_string() };
        services.insert(tmstmp_id, service_cfg.to_json());

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
        //warn!("actual cfg: {:?}", sandbox.cfg());
        assert_ne!(new_cfg, sandbox.cfg());

        let vote3 = TxConfigVote::new(&sandbox.p(2), 5, &propose_hash, 1700, false, sandbox.s(2));
        add_one_height_with_transactions(&sandbox, &sandbox_state, &[vote3.raw().clone()]);
        let view = sandbox.blockchain_copy().view();
        let schema = ConfigurationSchema::new(&view);
        let votes = schema.config_votes();
        sandbox.assert_state(8, 1);
        assert_eq!(vote3, votes.get(&sandbox.p(2)).unwrap().unwrap());
        assert_eq!(new_cfg, sandbox.cfg());
    }
}
