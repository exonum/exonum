use std::default::Default;

use time;
use time::Timespec;

use ::blockchain::Blockchain;
use ::crypto::{PublicKey, SecretKey, Hash, gen_keypair};
use ::messages::{Message, AnyTx, ServiceTx, ConfigPropose, ConfigMessage};
use super::config::{StoredConfiguration, ConsensusConfig};

pub struct GenesisBlock<B>
    where B: Blockchain
{
    pub time: Timespec,
    // pub txs: Vec<ServiceTx>, //?
    pub txs: Vec<(Hash, AnyTx<B::Transaction>)>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct GenesisConfig {
    pub time: u64,
    pub consensus: ConsensusConfig,
    pub validators: Vec<PublicKey>,
    pub public_key: PublicKey,
    pub secret_key: SecretKey,
}

impl GenesisConfig {
    pub fn new<I: Iterator<Item = PublicKey>>(validators: I) -> GenesisConfig {
        let (pub_key, sec_key) = gen_keypair();
        GenesisConfig {
            time: time::now_utc().to_timespec().sec as u64,
            validators: validators.collect::<Vec<_>>(),
            consensus: ConsensusConfig::default(),
            public_key: pub_key,
            secret_key: sec_key,
        }
    }
}

impl<B> Into<GenesisBlock<B>> for GenesisConfig
    where B: Blockchain
{
    fn into(self) -> GenesisBlock<B> {
        let configuration = StoredConfiguration {
            actual_from: 0,
            validators: self.validators,
            consensus: self.consensus,
        };
        let config_propose = ConfigPropose::new(&self.public_key,
                                                0,
                                                configuration.serialize().as_ref(),
                                                0,
                                                &self.secret_key);

        let txs = vec![
            (config_propose.hash(), AnyTx::Service(ServiceTx::ConfigChange(ConfigMessage::ConfigPropose(config_propose))))
        ];

        GenesisBlock {
            time: Timespec {
                sec: self.time as i64,
                nsec: 0,
            },
            txs: txs,
        }
    }
}