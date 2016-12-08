extern crate serde_json;

#[macro_use]
mod spec;
mod block;
mod view;
mod config;

use std::borrow::Borrow;
use std::ops::Deref;

use time::Timespec;

use byteorder::{ByteOrder, BigEndian};

use ::crypto::{PublicKey, Hash, hash};
use ::messages::{Any, Precommit, Message, ConfigMessage, ServiceTransaction, ConfigPropose, ConfigVote, TransactionMessage, RawMessage, AnyTx};
use ::storage::{StorageValue, Patch, Database, Fork, Error, Map, List};

pub use self::block::Block;
pub use self::view::{View, ConfigurationData, HeightBytecode};
pub use self::view::ConfigurationData;
pub use self::config::{StoredConfiguration, ConsensusCfg};

pub trait Blockchain: Sized + Clone + Send + Sync + 'static
    where Self: Deref<Target = <Self as Blockchain>::Database>
{
    type View: View<<<Self as Blockchain>::Database as Database>::Fork, Transaction=Self::Transaction>;
    type Database: Database;
    type Transaction: Message + StorageValue;

    fn last_hash(&self) -> Result<Option<Hash>, Error> {
        self.view().heights().last()
    }

    fn last_block(&self) -> Result<Option<Block>, Error> {
        let view = self.view();
        Ok(match view.heights().last()? {
            Some(hash) => Some(view.blocks().get(&hash)?.unwrap()),
            None => None,
        })
    }

    fn verify_tx(tx: &Self::Transaction) -> bool;
    fn state_hash(fork: &Self::View) -> Result<Hash, Error>;
    fn execute(fork: &Self::View, tx: &Self::Transaction) -> Result<(), Error>;
    // FIXME make private
    fn execute_service_tx(_: &Self::View, _: &ServiceTx) -> Result<(), Error> {
        unimplemented!();
    }

    // TODO use Iterator to avoid memory allocations?
    fn create_patch(&self,
                    height: u64,
                    round: u32,
                    time: Timespec,
                    txs: &[(Hash, AnyTx<Self::Transaction>)])
                    -> Result<(Hash, Vec<Hash>, Patch), Error> {
        // Get last hash
        let last_hash = self.last_hash()?.unwrap_or_else(|| hash(&[]));
        // Create fork
        let fork = self.view();
        // Save & execute transactions
        let mut tx_hashes = Vec::new();
        for &(hash, ref tx) in txs {
            
            match *tx {
                AnyTx::Application(ref tx) => Self::execute(&fork, tx)?,
                AnyTx::Service(ref tx) => Self::execute_service_tx(&fork, tx)?,
            }

            fork.transactions()
                .put(&hash, tx.clone())
                .unwrap();
            fork.block_txs(height)
                .append(hash)
                .unwrap();
            tx_hashes.push(hash);            
        }
        // Get tx hash
        let tx_hash = fork.block_txs(height).root_hash()?;
        // Get state hash
        let state_hash = Self::state_hash(&fork)?;
        // Create block
        let block = Block::new(height, round, time, &last_hash, &tx_hash, &state_hash);
        // Eval block hash
        let block_hash = block.hash();
        // Update height
        // TODO: check that height == propose.height
        fork.heights().append(block_hash).is_ok();
        // Save block
        fork.blocks().put(&block_hash, block).is_ok();
        Ok((block_hash, tx_hashes, fork.changes()))
    }

    fn execute_service_tx(&self, tx: ServiceTransaction) {
        match tx {
            ServiceTransaction::ConfigChange(config_message) => {
                self.execute_config_change(config_message);
            }            
        }
    }

    fn execute_config_change(&self, config_message: ConfigMessage) {
        match config_message {
            ConfigMessage::ConfigPropose(config_propose_tx) => {
                self.handle_config_propose(config_propose_tx);
            }
            ConfigMessage::ConfigVote(config_vote_tx) => {
                self.handle_config_vote(config_vote_tx);
            }
        }
    }

    fn get_height(&self) -> u64 {
        let r = self.last_block().unwrap();
        if let Some(last_block) = r {
            last_block.height() + 1
        } else {
            0
        }
    }

    fn get_actual_configuration(&self) -> Option<StoredConfiguration> {

        let h = self.get_height();

        let view = self.view();

        let heights = view.configs_heights();

        if let Ok(height_values) = heights.values() {

            if let Some(idx) = height_values.into_iter()
                .rposition(|r| u64::from(r) <= h) {
                if let Ok(Some(height)) = heights.get(idx as u64) {
                    return self.get_configuration_at_height(height.into());
                }
            }
        }

        None
    }

    fn get_configuration_at_height(&self, height: u64) -> Option<StoredConfiguration> {
        let view = self.view();
        let configs = view.configs();
        if let Ok(config) = configs.get(&height.into()) {
            match StoredConfiguration::deserialize(&config.unwrap()) {
                Ok(configuration) => {
                    return Some(configuration);
                }
                Err(_) => {
                    error!("Can't parse found configuration at height: {}", height);
                }
            }
        }
        None
    }

    fn handle_config_propose(&self, config_propose: ConfigPropose) {

        if let Some(config) = self.get_actual_configuration() {
            if !config.validators.contains(config_propose.from()) {
                error!("ConfigPropose from unknown validator: {:?}",
                       config_propose.from());
                return;
            }

            let view = self.view();
            let hash = <ConfigPropose as Message>::hash(&config_propose);
            if view.config_proposes().get(&hash).unwrap().is_some() {
                error!("Received config_propose has already been handled, msg={:?}",
                       config_propose);
                return;
            }

            trace!("Handle ConfigPropose");
            let _ = view.config_proposes().put(&hash, config_propose);
        }
    }

    fn handle_config_vote(&self, config_vote: ConfigVote) {

        if let Some(config) = self.get_actual_configuration() {

            if !config.validators.contains(config_vote.from()) {
                error!("ConfigVote from unknown validator: {:?}",
                       config_vote.from());
                return;
            }

            let view = self.view();
            if view.config_proposes().get(config_vote.hash_propose()).unwrap().is_some() {
                error!("Received config_vote for unknown transaciton, msg={:?}",
                       config_vote);
                return;
            }

            if let Some(vote) = view.config_votes().get(config_vote.from()).unwrap() {
                if vote.seed() != config_vote.seed() - 1 {
                    error!("Received config_vote with wrong seed, msg={:?}",
                           config_vote);
                    return;
                }
            }

            let msg = config_vote.clone();
            let _ = view.config_votes().put(msg.from(), config_vote.clone());

            let mut votes_count = 0;
            for pub_key in config.validators.clone() {
                if let Some(vote) = view.config_votes().get(&pub_key).unwrap() {
                    if !vote.revoke() {
                        votes_count += 1;
                    }
                }
            }

            if votes_count >= 2 / 3 * config.validators.len() {
                if let Some(config_propose) = view.config_proposes()
                    .get(config_vote.hash_propose())
                    .unwrap() {
                    let height_bytecode = config_propose.actual_from_height().into();
                    view.configs().put(&height_bytecode, config_propose.config().to_vec()).unwrap();
                    view.configs_heights().append(height_bytecode).unwrap();
                    // TODO: clear storages
                }
            }
        }
    }

    fn commit<'a, I: Iterator<Item = &'a Precommit>>(&self,
                                                     block_hash: Hash,
                                                     patch: &Patch,
                                                     precommits: I)
                                                     -> Result<(), Error> {
        let patch = {
            let view = self.view();
            view.merge(patch);

            for precommit in precommits {
                view.precommits(&block_hash).append(precommit.clone())?;
            }

            view.changes()
        };

        self.merge(&patch)
    }

    fn view(&self) -> Self::View {
        Self::View::from_fork(self.borrow().fork())
    }

    fn merge(&self, patch: &Patch) -> Result<(), Error> {
        self.deref().merge(patch)
    }
}
