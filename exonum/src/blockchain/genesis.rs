use ::crypto::PublicKey;
use super::config::ConsensusConfig;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct GenesisConfig {
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
            validators: validators.collect::<Vec<_>>(),
            consensus: consensus,
        }
    }
}
