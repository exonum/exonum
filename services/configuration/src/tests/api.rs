// Copyright 2018 The Exonum Team
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

// spell-checker:ignore postpropose, postvote

use exonum::blockchain::{Schema, StoredConfiguration};
use exonum::crypto::{CryptoHash, Hash};
use exonum::helpers::{Height, ValidatorId};
use exonum_testkit::{ApiKind, TestKit, TestKitApi};

use super::{new_tx_config_propose, new_tx_config_vote, new_tx_config_vote_against, to_boxed,
            ConfigurationSchema, ConfigurationTestKit};
use api::{ConfigHashInfo, ConfigInfo, ProposeHashInfo, ProposeResponse, VoteResponse, VotesInfo};

trait ConfigurationApiTest {
    fn actual_config(&self) -> ConfigHashInfo;

    fn following_config(&self) -> Option<ConfigHashInfo>;

    fn config_by_hash(&self, config_hash: Hash) -> ConfigInfo;

    fn all_proposes(
        &self,
        previous_cfg_hash_filter: Option<Hash>,
        actual_from_filter: Option<Height>,
    ) -> Vec<ProposeHashInfo>;

    fn all_committed(
        &self,
        previous_cfg_hash_filter: Option<Hash>,
        actual_from_filter: Option<Height>,
    ) -> Vec<ConfigHashInfo>;

    fn votes_for_propose(&self, cfg_hash: &Hash) -> VotesInfo;

    fn post_config_propose(&self, cfg: &StoredConfiguration) -> ProposeResponse;

    fn post_config_vote(&self, cfg_hash: &Hash) -> VoteResponse;

    fn post_config_vote_against(&self, cfg_hash: &Hash) -> VoteResponse;
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
    fn actual_config(&self) -> ConfigHashInfo {
        self.get(ApiKind::Service("configuration"), "/v1/configs/actual")
    }

    fn following_config(&self) -> Option<ConfigHashInfo> {
        self.get(ApiKind::Service("configuration"), "/v1/configs/following")
    }

    fn config_by_hash(&self, config_hash: Hash) -> ConfigInfo {
        self.get(
            ApiKind::Service("configuration"),
            &format!("/v1/configs/{}", config_hash.to_string()),
        )
    }

    fn all_proposes(
        &self,
        previous_cfg_hash_filter: Option<Hash>,
        actual_from_filter: Option<Height>,
    ) -> Vec<ProposeHashInfo> {
        self.get(
            ApiKind::Service("configuration"),
            &format!(
                "/v1/configs/proposed{}",
                &params_to_query(previous_cfg_hash_filter, actual_from_filter),
            ),
        )
    }

    fn votes_for_propose(&self, cfg_hash: &Hash) -> VotesInfo {
        let endpoint = format!("/v1/configs/{}/votes", cfg_hash.to_string());
        self.get(ApiKind::Service("configuration"), &endpoint)
    }

    fn all_committed(
        &self,
        previous_cfg_hash_filter: Option<Hash>,
        actual_from_filter: Option<Height>,
    ) -> Vec<ConfigHashInfo> {
        self.get(
            ApiKind::Service("configuration"),
            &format!(
                "/v1/configs/committed{}",
                &params_to_query(previous_cfg_hash_filter, actual_from_filter),
            ),
        )
    }

    fn post_config_propose(&self, cfg: &StoredConfiguration) -> ProposeResponse {
        self.post_private(
            ApiKind::Service("configuration"),
            "/v1/configs/postpropose",
            cfg,
        )
    }

    fn post_config_vote(&self, cfg_hash: &Hash) -> VoteResponse {
        let endpoint = format!("/v1/configs/{}/postvote", cfg_hash.to_string());
        self.post_private(ApiKind::Service("configuration"), &endpoint, &())
    }

    fn post_config_vote_against(&self, cfg_hash: &Hash) -> VoteResponse {
        let endpoint = format!("/v1/configs/{}/postagainst", cfg_hash.to_string());
        self.post_private(ApiKind::Service("configuration"), &endpoint, &())
    }
}

#[test]
fn test_actual_config() {
    let testkit: TestKit = TestKit::configuration_default();

    let actual = testkit.api().actual_config();
    let expected = {
        let stored = Schema::new(&testkit.snapshot()).actual_configuration();
        ConfigHashInfo {
            hash: stored.hash(),
            config: stored,
            propose: None,
            votes: None,
        }
    };
    assert_eq!(expected, actual);
}

#[test]
fn test_following_config() {
    let mut testkit: TestKit = TestKit::configuration_default();
    let api = testkit.api();
    // Checks that following config is absent.
    assert_eq!(None, api.following_config());
    // Commits the following configuration.
    let cfg_proposal = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_actual_from(Height(10));
        cfg.set_service_config("message", "First config change");
        cfg
    };
    let expected = {
        let stored = cfg_proposal.stored_configuration().clone();
        ConfigHashInfo {
            hash: stored.hash(),
            config: stored,
            propose: None,
            votes: None,
        }
    };
    testkit.commit_configuration_change(cfg_proposal);
    testkit.create_block();

    let actual = api.following_config();
    assert_eq!(Some(expected), actual);
}

#[test]
fn test_config_by_hash1() {
    let testkit: TestKit = TestKit::configuration_default();
    let initial_cfg = Schema::new(&testkit.snapshot()).actual_configuration();

    let expected = ConfigInfo {
        committed_config: Some(initial_cfg.clone()),
        propose: None,
    };
    let actual = testkit.api().config_by_hash(initial_cfg.hash());
    assert_eq!(expected, actual);
}

#[test]
fn test_config_by_hash2() {
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
    let expected = ConfigInfo {
        committed_config: Some(new_cfg.clone()),
        propose: Some(
            ConfigurationSchema::new(&testkit.snapshot())
                .propose_data_by_config_hash()
                .get(&new_cfg.hash())
                .expect("Propose for configuration is absent."),
        ),
    };
    let actual = testkit.api().config_by_hash(new_cfg.hash());
    assert_eq!(expected, actual);
}

#[test]
fn test_config_by_hash3() {
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
    let expected = ConfigInfo {
        committed_config: Some(new_cfg.clone()),
        propose: Some(
            ConfigurationSchema::new(&testkit.snapshot())
                .propose_data_by_config_hash()
                .get(&new_cfg.hash())
                .expect("Propose for configuration is absent."),
        ),
    };
    let actual = testkit.api().config_by_hash(new_cfg.hash());
    assert_eq!(expected, actual);
}

#[test]
fn test_votes_for_propose() {
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
    assert_eq!(None, api.votes_for_propose(&new_cfg.hash()));
    testkit.create_block_with_transactions(txvec![tx_propose]);
    assert_eq!(
        Some(vec![None; testkit.network().validators().len()]),
        api.votes_for_propose(&new_cfg.hash())
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
    let response = api.votes_for_propose(&new_cfg.hash())
        .expect("Votes for config is absent");
    for entry in response.into_iter().take(testkit.majority_count()) {
        let tx = entry.expect("Vote for config is absent");
        assert!(
            Schema::new(&testkit.snapshot())
                .transactions()
                .contains(&tx.hash(),),
            "Transaction is absent in blockchain: {:?}",
            tx
        );
    }
}

#[test]
fn test_dissenting_votes_for_propose() {
    use schema::VotingDecision;

    let mut testkit: TestKit = TestKit::configuration_default();
    let api = testkit.api();
    // Apply a new configuration.
    let new_cfg = {
        let mut cfg = testkit.configuration_change_proposal();
        cfg.set_service_config("message", "First config change");
        cfg.set_actual_from(Height(5));
        cfg.stored_configuration().clone()
    };
    let tx_propose = new_tx_config_propose(&testkit.network().validators()[0], new_cfg.clone());
    let cfg_proposal_hash = new_cfg.hash();
    assert_eq!(None, api.votes_for_propose(&new_cfg.hash()));
    testkit.create_block_with_transaction(tx_propose);
    assert_eq!(
        Some(vec![None; testkit.network().validators().len()]),
        api.votes_for_propose(&new_cfg.hash())
    );
    // Push dissenting votes
    let tx_dissenting_votes = testkit
        .network()
        .validators()
        .iter()
        .map(|validator| new_tx_config_vote_against(validator, cfg_proposal_hash))
        .map(to_boxed)
        .collect::<Vec<_>>();
    testkit.create_block_with_transactions(tx_dissenting_votes);
    let response = api.votes_for_propose(&new_cfg.hash())
        .expect("Dissenting votes for config is absent");
    for entry in response.into_iter().take(testkit.majority_count()) {
        let tx = entry.expect("VoteAgainst for config is absent");
        assert_matches!(
            tx,
            VotingDecision::Nay(_),
            "Transaction {:?} is not VoteAgainst variant",
            tx
        );
    }
}

#[test]
fn test_all_proposes() {
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
    let expected_response_1 = ProposeHashInfo {
        hash: new_cfg_1.hash(),
        propose_data: ConfigurationSchema::new(&testkit.snapshot())
            .propose_data_by_config_hash()
            .get(&new_cfg_1.hash())
            .expect("Propose data is absent"),
    };
    let expected_response_2 = ProposeHashInfo {
        hash: new_cfg_2.hash(),
        propose_data: ConfigurationSchema::new(&testkit.snapshot())
            .propose_data_by_config_hash()
            .get(&new_cfg_2.hash())
            .expect("Propose data is absent"),
    };
    assert_eq!(
        vec![expected_response_1.clone(), expected_response_2.clone()],
        api.all_proposes(None, None)
    );
    assert_eq!(
        vec![expected_response_2.clone()],
        api.all_proposes(None, Some(Height(11)))
    );
    assert_eq!(
        vec![expected_response_2.clone()],
        api.all_proposes(None, Some(Height(15)))
    );
    assert_eq!(
        Vec::<ProposeHashInfo>::new(),
        api.all_proposes(None, Some(Height(16)))
    );
    assert_eq!(
        Vec::<ProposeHashInfo>::new(),
        api.all_proposes(Some(Hash::zero()), None)
    );
    let initial_cfg = Schema::new(&testkit.snapshot()).actual_configuration();
    assert_eq!(
        vec![expected_response_1.clone(), expected_response_2.clone()],
        api.all_proposes(Some(initial_cfg.hash()), None)
    );
}

#[test]
fn test_all_committed() {
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
    let expected_response_1 = ConfigHashInfo {
        hash: initial_cfg.hash(),
        config: initial_cfg.clone(),
        propose: None,
        votes: None,
    };
    let expected_response_2 = ConfigHashInfo {
        hash: new_cfg_1.hash(),
        config: new_cfg_1.clone(),
        propose: Some(testkit.find_propose(new_cfg_1.hash()).unwrap().hash()),
        votes: Some(testkit.votes_for_propose(new_cfg_1.hash())),
    };
    let expected_response_3 = ConfigHashInfo {
        hash: new_cfg_2.hash(),
        config: new_cfg_2.clone(),
        propose: Some(testkit.find_propose(new_cfg_2.hash()).unwrap().hash()),
        votes: Some(testkit.votes_for_propose(new_cfg_2.hash())),
    };
    assert_eq!(
        vec![
            expected_response_1.clone(),
            expected_response_2.clone(),
            expected_response_3.clone(),
        ],
        api.all_committed(None, None)
    );
    assert_eq!(
        vec![expected_response_2.clone(), expected_response_3.clone()],
        api.all_committed(None, Some(Height(1)))
    );
    assert_eq!(
        vec![expected_response_3.clone()],
        api.all_committed(None, Some(Height(11)))
    );
    assert_eq!(
        vec![expected_response_3.clone()],
        api.all_committed(None, Some(Height(15)))
    );
    assert_eq!(
        Vec::<ConfigHashInfo>::new(),
        api.all_committed(None, Some(Height(16)))
    );
    assert_eq!(
        vec![expected_response_1.clone()],
        api.all_committed(Some(Hash::zero()), None)
    );
    assert_eq!(
        vec![expected_response_2.clone()],
        api.all_committed(Some(initial_cfg.hash()), None)
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
    assert!(testkit.is_tx_in_pool(&info.tx_hash));
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
    assert!(testkit.is_tx_in_pool(&info.tx_hash));
}

#[test]
fn test_post_vote_against_tx() {
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
    testkit.create_block_with_transaction(tx);

    let info = api.post_config_vote_against(&new_cfg.hash());
    testkit.poll_events();
    // Check results
    let tx = new_tx_config_vote_against(&testkit.network().validators()[0], new_cfg.hash());
    assert_eq!(tx.hash(), info.tx_hash);
    assert!(testkit.is_tx_in_pool(&info.tx_hash));
}
