use time;

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
            time: time::now_utc().to_timespec().sec as u64,
            validators: validators.collect::<Vec<_>>(),
            consensus: consensus,
        }
    }
}
