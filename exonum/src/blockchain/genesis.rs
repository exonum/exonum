use crypto::PublicKey;

use super::config::ConsensusConfig;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct GenesisConfig {
    pub consensus: ConsensusConfig,
    pub validator_keys: Vec<PublicKey>,
    pub service_keys: Vec<PublicKey>,
}

impl GenesisConfig {
    pub fn new<I1, I2>(validators: I1, services: I2) -> Self
        where I1: Iterator<Item = PublicKey>, I2: Iterator<Item = PublicKey>
    {
        Self::new_with_consensus(ConsensusConfig::default(), validators, services)
    }

    pub fn new_with_consensus<I1, I2>(consensus: ConsensusConfig,
                                      validator_keys: I1,
                                      service_keys: I2) -> Self
        where I1: Iterator<Item = PublicKey>, I2: Iterator<Item = PublicKey>
    {
        GenesisConfig {
            consensus: consensus,
            validator_keys: validator_keys.collect(),
            service_keys: service_keys.collect(),
        }
    }
}
