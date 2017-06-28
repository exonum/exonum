use super::config::{ConsensusConfig, ValidatorKeys};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct GenesisConfig {
    pub consensus: ConsensusConfig,
    pub validator_keys: Vec<ValidatorKeys>,
}

impl GenesisConfig {
    pub fn new<I: Iterator<Item = ValidatorKeys>>(validators: I) -> Self
    {
        Self::new_with_consensus(ConsensusConfig::default(), validators)
    }

    pub fn new_with_consensus<I>(consensus: ConsensusConfig, validator_keys: I) -> Self
        where I: Iterator<Item = ValidatorKeys>
    {
        GenesisConfig {
            consensus: consensus,
            validator_keys: validator_keys.collect(),
        }
    }
}
