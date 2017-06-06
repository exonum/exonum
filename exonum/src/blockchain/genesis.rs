use crypto::PublicKey;

use super::config::ConsensusConfig;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct GenesisConfig {
    pub consensus: ConsensusConfig,
    pub validators: Vec<(PublicKey, PublicKey)>,
}

impl GenesisConfig {
    pub fn new<I: Iterator<Item = (PublicKey, PublicKey)>>(validators: I) -> Self {
        Self::new_with_consensus(ConsensusConfig::default(), validators)
    }

    pub fn new_with_consensus<I>(consensus: ConsensusConfig, validators: I) -> Self
        where I: Iterator<Item = (PublicKey, PublicKey)>
    {
        GenesisConfig {
            validators: validators.collect::<Vec<_>>(),
            consensus: consensus,
        }
    }
}
