use crypto::PublicKey;

use super::config::ConsensusConfig;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct GenesisConfig {
    pub consensus: ConsensusConfig,
    pub validators: Vec<PublicKey>,
    pub services: Vec<PublicKey>,
}

impl GenesisConfig {
    pub fn new<I1, I2>(validators: I1, services: I2) -> Self
        where I1: Iterator<Item = PublicKey>, I2: Iterator<Item = PublicKey>
    {
        Self::new_with_consensus(ConsensusConfig::default(), validators, services)
    }

    pub fn new_with_consensus<I1, I2>(consensus: ConsensusConfig,
                                      validators: I1,
                                      services: I2) -> Self
        where I1: Iterator<Item = PublicKey>, I2: Iterator<Item = PublicKey>
    {
        GenesisConfig {
            consensus: consensus,
            validators: validators.collect(),
            services: services.collect(),
        }
    }
}
