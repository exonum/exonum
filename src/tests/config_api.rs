use exonum::blockchain::{Schema, StoredConfiguration};
use exonum::helpers::{Height, ValidatorId};
use exonum::storage::StorageValue;
use exonum::crypto::Hash;
use exonum_testkit::{ApiKind, TestKit, TestKitApi};

use ConfigurationSchema;
use config_api::{ApiResponseConfigHashInfo, ApiResponseConfigInfo, ApiResponseProposeHashInfo,
                 ApiResponseProposePost, ApiResponseVotePost, ApiResponseVotesInfo};
use super::{new_tx_config_propose, new_tx_config_vote, to_boxed, ConfigurationTestKit};

trait ConfigurationApiTest {
    fn get_actual_config(&self) -> ApiResponseConfigHashInfo;

    fn get_following_config(&self) -> Option<ApiResponseConfigHashInfo>;

    fn get_config_by_hash(&self, config_hash: Hash) -> ApiResponseConfigInfo;

    fn get_all_proposes(
        &self,
        previous_cfg_hash_filter: Option<Hash>,
        actual_from_filter: Option<Height>,
    ) -> Vec<ApiResponseProposeHashInfo>;

    fn get_all_committed(
        &self,
        previous_cfg_hash_filter: Option<Hash>,
        actual_from_filter: Option<Height>,
    ) -> Vec<ApiResponseConfigHashInfo>;

    fn get_votes_for_propose(&self, cfg_hash: &Hash) -> ApiResponseVotesInfo;

    fn post_config_propose(&self, cfg: &StoredConfiguration) -> ApiResponseProposePost;

    fn post_config_vote(&self, cfg_hash: &Hash) -> ApiResponseVotePost;
}

fn params_to_query(
    previous_cfg_hash_filter: Option<Hash>,
    actual_from_filter: Option<Height>,
) -> String {
    let mut query = String::new();
    let mut prefix = "?";
    if let Some(previous_cfg_hash_filter) = previous_cfg_hash_filter {
        query += &format!(
            "{}previous_cfg_hash={}",
            prefix,
            previous_cfg_hash_filter.to_string()
        );
        prefix = "&";
    }
    if let Some(actual_from_filter) = actual_from_filter {
        query += &format!("{}actual_from={}", prefix, actual_from_filter.to_string());
    }
    query
}

impl ConfigurationApiTest for TestKitApi {
    fn get_actual_config(&self) -> ApiResponseConfigHashInfo {
        self.get(ApiKind::Service("configuration"), "/v1/configs/actual")
    }

    fn get_following_config(&self) -> Option<ApiResponseConfigHashInfo> {
        self.get(ApiKind::Service("configuration"), "/v1/configs/following")
    }

    fn get_config_by_hash(&self, config_hash: Hash) -> ApiResponseConfigInfo {
        self.get(
            ApiKind::Service("configuration"),
            &format!("/v1/configs/{}", config_hash.to_string()),
        )
    }

    fn get_all_proposes(
        &self,
        previous_cfg_hash_filter: Option<Hash>,
        actual_from_filter: Option<Height>,
    ) -> Vec<ApiResponseProposeHashInfo> {
        self.get(
            ApiKind::Service("configuration"),
            &format!(
                "/v1/configs/proposed{}",
                &params_to_query(previous_cfg_hash_filter, actual_from_filter),
            ),
        )
    }

    fn get_votes_for_propose(&self, cfg_hash: &Hash) -> ApiResponseVotesInfo {
        let endpoint = format!("/v1/configs/{}/votes", cfg_hash.to_string());
        self.get(ApiKind::Service("configuration"), &endpoint)
    }

    fn get_all_committed(
        &self,
        previous_cfg_hash_filter: Option<Hash>,
        actual_from_filter: Option<Height>,
    ) -> Vec<ApiResponseConfigHashInfo> {
        self.get(
            ApiKind::Service("configuration"),
            &format!(
                "/v1/configs/committed{}",
                &params_to_query(previous_cfg_hash_filter, actual_from_filter),
            ),
        )
    }

    fn post_config_propose(&self, cfg: &StoredConfiguration) -> ApiResponseProposePost {
        self.post_private(
            ApiKind::Service("configuration"),
            "/v1/configs/postpropose",
            cfg,
        )
    }

    fn post_config_vote(&self, cfg_hash: &Hash) -> ApiResponseVotePost {
        let endpoint = format!("/v1/configs/{}/postvote", cfg_hash.to_string());
        self.post_private(ApiKind::Service("configuration"), &endpoint, &())
    }
}


#[test]
fn test_get_actual_config() {
    let testkit: TestKit = TestKit::configuration_default();

    let actual = testkit.api().get_actual_config();
    let expected = {
        let stored = Schema::new(&testkit.snapshot()).actual_configuration();
        ApiResponseConfigHashInfo {
            hash: stored.hash(),
            config: stored,
        }
    };
    assert_eq!(expected, actual);
}

#[test]
fn test_get_following_config() {
    let mut testkit: TestKit = TestKit::configuration_default();
    let api = testkit.api();
    // Checks that following config is absent.
    assert_eq!(None, api.get_following_config());
    // Commits the following configuration.
    let cfg_proposal = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_actual_from(Height(10));
        cfg.set_service_config("message", "First config change");
        cfg
    };
    let expected = {
        let stored = cfg_proposal.stored_configuration().clone();
        ApiResponseConfigHashInfo {
            hash: stored.hash(),
            config: stored,
        }
    };
    testkit.commit_configuration_change(cfg_proposal);
    testkit.create_block();

    let actual = api.get_following_config();
    assert_eq!(Some(expected), actual);
}

#[test]
fn test_get_config_by_hash1() {
    let testkit: TestKit = TestKit::configuration_default();
    let initial_cfg = Schema::new(&testkit.snapshot()).actual_configuration();

    let expected = ApiResponseConfigInfo {
        committed_config: Some(initial_cfg.clone()),
        propose: None,
    };
    let actual = testkit.api().get_config_by_hash(initial_cfg.hash());
    assert_eq!(expected, actual);
}

#[test]
fn test_get_config_by_hash2() {
    let mut testkit: TestKit = TestKit::configuration_default();
    // Apply a new configuration.
    let new_cfg = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_service_config("message", "First config change");
        cfg.set_actual_from(Height(5));
        cfg.stored_configuration().clone()
    };
    testkit.apply_configuration(ValidatorId(0), new_cfg.clone());
    // Check results
    let expected = ApiResponseConfigInfo {
        committed_config: Some(new_cfg.clone()),
        propose: Some(
            ConfigurationSchema::new(&testkit.snapshot())
                .propose_data_by_config_hash()
                .get(&new_cfg.hash())
                .expect("Propose for configuration is absent."),
        ),
    };
    let actual = testkit.api().get_config_by_hash(new_cfg.hash());
    assert_eq!(expected, actual);
}

#[test]
fn test_get_config_by_hash3() {
    let mut testkit: TestKit = TestKit::configuration_default();
    // Apply a new configuration.
    let new_cfg = {
        let mut cfg = testkit.configuration_change_proposal();
        let mut validators = cfg.validators().to_vec();
        validators.pop();
        cfg.set_actual_from(Height(5));
        cfg.set_validators(validators);
        cfg.stored_configuration().clone()
    };
    testkit.apply_configuration(ValidatorId(0), new_cfg.clone());
    // Check results
    let expected = ApiResponseConfigInfo {
        committed_config: Some(new_cfg.clone()),
        propose: Some(
            ConfigurationSchema::new(&testkit.snapshot())
                .propose_data_by_config_hash()
                .get(&new_cfg.hash())
                .expect("Propose for configuration is absent."),
        ),
    };
    let actual = testkit.api().get_config_by_hash(new_cfg.hash());
    assert_eq!(expected, actual);
}

#[test]
fn test_get_votes_for_propose() {
    let mut testkit: TestKit = TestKit::configuration_default();
    let api = testkit.api();
    // Apply a new configuration.
    let cfg_change_height = Height(5);
    let new_cfg = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_service_config("message", "First config change");
        cfg.set_actual_from(cfg_change_height);
        cfg.stored_configuration().clone()
    };
    let tx_propose = new_tx_config_propose(&testkit.network().validators()[0], new_cfg.clone());
    let cfg_proposal_hash = new_cfg.hash();
    assert_eq!(None, api.get_votes_for_propose(&new_cfg.hash()));
    testkit.create_block_with_transactions(txvec![tx_propose]);
    assert_eq!(
        Some(vec![None; testkit.network().validators().len()]),
        api.get_votes_for_propose(&new_cfg.hash())
    );
    // Push votes
    let tx_votes = testkit
        .network()
        .validators()
        .iter()
        .map(|validator| new_tx_config_vote(validator, cfg_proposal_hash))
        .map(to_boxed)
        .collect::<Vec<_>>();
    testkit.create_block_with_transactions(tx_votes);
    let response = api.get_votes_for_propose(&new_cfg.hash()).expect(
        "Votes for config is absent",
    );
    for entry in response.into_iter().take(testkit.majority_count()) {
        let tx = entry.expect("Vote for config is absent");
        assert!(
            Schema::new(&testkit.snapshot()).transactions().contains(
                &tx.hash(),
            ),
            "Transaction is absent in blockchain: {:?}",
            tx
        );
    }
}

#[test]
fn test_get_all_proposes() {
    let mut testkit: TestKit = TestKit::configuration_default();
    let api = testkit.api();
    // Create proposes
    let new_cfg_1 = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_actual_from(Height(10));
        cfg.set_service_config("message", "First config change");
        cfg.stored_configuration().clone()
    };
    let new_cfg_2 = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_actual_from(Height(15));
        cfg.set_service_config("message", "First config change");
        cfg.stored_configuration().clone()
    };
    // Create txs
    let tx_propose_1 = new_tx_config_propose(&testkit.network().validators()[0], new_cfg_1.clone());
    let tx_propose_2 = new_tx_config_propose(&testkit.network().validators()[1], new_cfg_2.clone());
    testkit.create_block_with_transactions(txvec![tx_propose_1, tx_propose_2]);
    // Check results
    let expected_response_1 = ApiResponseProposeHashInfo {
        hash: new_cfg_1.hash(),
        propose_data: ConfigurationSchema::new(&testkit.snapshot())
            .propose_data_by_config_hash()
            .get(&new_cfg_1.hash())
            .expect("Propose data is absent"),
    };
    let expected_response_2 = ApiResponseProposeHashInfo {
        hash: new_cfg_2.hash(),
        propose_data: ConfigurationSchema::new(&testkit.snapshot())
            .propose_data_by_config_hash()
            .get(&new_cfg_2.hash())
            .expect("Propose data is absent"),
    };
    assert_eq!(
        vec![expected_response_1.clone(), expected_response_2.clone()],
        api.get_all_proposes(None, None)
    );
    assert_eq!(
        vec![expected_response_2.clone()],
        api.get_all_proposes(None, Some(Height(11)))
    );
    assert_eq!(
        vec![expected_response_2.clone()],
        api.get_all_proposes(None, Some(Height(15)))
    );
    assert_eq!(
        Vec::<ApiResponseProposeHashInfo>::new(),
        api.get_all_proposes(None, Some(Height(16)))
    );
    assert_eq!(
        Vec::<ApiResponseProposeHashInfo>::new(),
        api.get_all_proposes(Some(Hash::zero()), None)
    );
    let initial_cfg = Schema::new(&testkit.snapshot()).actual_configuration();
    assert_eq!(
        vec![expected_response_1.clone(), expected_response_2.clone()],
        api.get_all_proposes(Some(initial_cfg.hash()), None)
    );
}

#[test]
fn test_get_all_committed() {
    let mut testkit: TestKit = TestKit::configuration_default();
    let api = testkit.api();
    let initial_cfg = Schema::new(&testkit.snapshot()).actual_configuration();
    // Commits the first configuration
    let new_cfg_1 = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_actual_from(Height(10));
        cfg.set_service_config("message", "First config change");
        cfg.stored_configuration().clone()
    };
    testkit.apply_configuration(ValidatorId(0), new_cfg_1.clone());
    // Commits the first second configuration
    let new_cfg_2 = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_actual_from(Height(15));
        cfg.set_service_config("message", "First config change");
        cfg.stored_configuration().clone()
    };
    testkit.apply_configuration(ValidatorId(1), new_cfg_2.clone());
    // Check results
    let expected_response_1 = ApiResponseConfigHashInfo {
        hash: initial_cfg.hash(),
        config: initial_cfg.clone(),
    };
    let expected_response_2 = ApiResponseConfigHashInfo {
        hash: new_cfg_1.hash(),
        config: new_cfg_1.clone(),
    };
    let expected_response_3 = ApiResponseConfigHashInfo {
        hash: new_cfg_2.hash(),
        config: new_cfg_2.clone(),
    };
    assert_eq!(
        vec![
            expected_response_1.clone(),
            expected_response_2.clone(),
            expected_response_3.clone(),
        ],
        api.get_all_committed(None, None)
    );
    assert_eq!(
        vec![expected_response_2.clone(), expected_response_3.clone()],
        api.get_all_committed(None, Some(Height(1)))
    );
    assert_eq!(
        vec![expected_response_3.clone()],
        api.get_all_committed(None, Some(Height(11)))
    );
    assert_eq!(
        vec![expected_response_3.clone()],
        api.get_all_committed(None, Some(Height(15)))
    );
    assert_eq!(
        Vec::<ApiResponseConfigHashInfo>::new(),
        api.get_all_committed(None, Some(Height(16)))
    );
    assert_eq!(
        vec![expected_response_1.clone()],
        api.get_all_committed(Some(Hash::zero()), None)
    );
    assert_eq!(
        vec![expected_response_2.clone()],
        api.get_all_committed(Some(initial_cfg.hash()), None)
    );
}

#[test]
fn test_post_propose_tx() {
    let mut testkit: TestKit = TestKit::configuration_default();
    let api = testkit.api();
    // Commits the following configuration.
    let new_cfg = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_actual_from(Height(10));
        cfg.set_service_config("message", "First config change");
        cfg.stored_configuration().clone()
    };
    let info = api.post_config_propose(&new_cfg);
    testkit.poll_events();
    // Check results
    let tx = new_tx_config_propose(&testkit.network().validators()[0], new_cfg.clone());
    assert_eq!(tx.hash(), info.tx_hash);
    assert!(testkit.mempool().contains_key(&info.tx_hash));
}

#[test]
fn test_post_vote_tx() {
    let mut testkit: TestKit = TestKit::configuration_default();
    let api = testkit.api();
    // Commits the following configuration.
    let new_cfg = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_actual_from(Height(10));
        cfg.set_service_config("message", "First config change");
        cfg.stored_configuration().clone()
    };
    let tx = new_tx_config_propose(&testkit.network().validators()[0], new_cfg.clone());
    testkit.create_block_with_transactions(txvec![tx]);

    let info = api.post_config_vote(&new_cfg.hash());
    testkit.poll_events();
    // Check results
    let tx = new_tx_config_vote(&testkit.network().validators()[0], new_cfg.hash());
    assert_eq!(tx.hash(), info.tx_hash);
    assert!(testkit.mempool().contains_key(&info.tx_hash));
}
