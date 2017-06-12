use crypto::PublicKey;

use super::config::ConsensusConfig;

/// The initial `exonum-core` configuration which is committed into the genesis block.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct GenesisConfig {
    /// Configuration of consensus.
    pub consensus: ConsensusConfig,
    /// List of public keys for validators.
    pub validators: Vec<PublicKey>,
}

impl GenesisConfig {
    /// Creates a default configuration from the given list of public keys.
    pub fn new<I: Iterator<Item = PublicKey>>(validators: I) -> GenesisConfig {
        Self::new_with_consensus(ConsensusConfig::default(), validators)
    }

    /// Creates a configuration from the given consensus configuration and list of public keys.
    pub fn new_with_consensus<I: Iterator<Item = PublicKey>>(consensus: ConsensusConfig,
                                                             validators: I)
                                                             -> GenesisConfig {
        GenesisConfig {
            validators: validators.collect::<Vec<_>>(),
            consensus: consensus,
        }
    }
}
