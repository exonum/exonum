use super::config::{ConsensusConfig, ValidatorKeys};

/// The initial `exonum-core` configuration which is committed into the genesis block.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct GenesisConfig {
    /// Configuration of consensus.
    pub consensus: ConsensusConfig,
    /// List of public keys for validators.
    pub validator_keys: Vec<ValidatorKeys>,
}

impl GenesisConfig {
    /// Creates a default configuration from the given list of public keys.
    pub fn new<I: Iterator<Item = ValidatorKeys>>(validators: I) -> Self {
        Self::new_with_consensus(ConsensusConfig::default(), validators)
    }

    /// Creates a configuration from the given consensus configuration and list of public keys.
    pub fn new_with_consensus<I>(consensus: ConsensusConfig, validator_keys: I) -> Self
        where I: Iterator<Item = ValidatorKeys>
    {
        GenesisConfig {
            consensus: consensus,
            validator_keys: validator_keys.collect(),
        }
    }
}
