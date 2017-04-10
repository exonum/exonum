use std::time::{SystemTime, UNIX_EPOCH};
use ::crypto::PublicKey;
use super::config::ConsensusConfig;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct GenesisConfig {
    pub time: u64,
    pub consensus: ConsensusConfig,
    pub validators: Vec<PublicKey>,
}

impl GenesisConfig {
    pub fn new<I: Iterator<Item = PublicKey>>(validators: I) -> GenesisConfig {
        Self::new_with_consensus(ConsensusConfig::default(), validators)
    }

    pub fn new_with_consensus<I: Iterator<Item = PublicKey>>(consensus: ConsensusConfig,
                                                             validators: I)
                                                             -> GenesisConfig {
        GenesisConfig {
            time: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            validators: validators.collect::<Vec<_>>(),
            consensus: consensus,
        }
    }
}
