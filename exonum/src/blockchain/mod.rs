#[macro_use]
mod spec;
mod block;
mod view;
pub mod config;
mod genesis;

use std::borrow::Borrow;
use std::ops::Deref;

use time::Timespec;

use ::crypto::Hash;
use ::messages::{Precommit, Message, ConfigMessage, ServiceTx, ConfigPropose, ConfigVote, AnyTx};

use ::storage::{StorageValue, Patch, Database, Fork, Error, Map, List};
use self::genesis::GenesisBlock;

pub use self::block::Block;
pub use self::view::{View, ConfigurationData};
pub use self::genesis::GenesisConfig;
pub use self::config::{StoredConfiguration, ConsensusConfig};

pub trait Blockchain: Sized + Clone + Send + Sync + 'static
    where Self: Deref<Target = <Self as Blockchain>::Database>
{
    type View: View<<<Self as Blockchain>::Database as Database>::Fork, Transaction=Self::Transaction>;
    type Database: Database;
    type Transaction: Message + StorageValue;

    fn last_hash(&self) -> Result<Hash, Error> {
        Ok(self.view()
            .heights()
            .last()?
            .unwrap_or_else(|| Hash::default()))
    }

    fn last_block(&self) -> Result<Block, Error> {
        Ok(self.view().last_block()?.unwrap())
    }

    fn verify_tx(tx: &Self::Transaction) -> bool;
    fn state_hash(fork: &Self::View) -> Result<Hash, Error>;
    fn execute(fork: &Self::View, tx: &Self::Transaction) -> Result<(), Error>;
    // FIXME make private
    fn execute_service_tx(view: &Self::View, tx: &ServiceTx) -> Result<(), Error> {
        match *tx {
            ServiceTx::ConfigChange(ref config_message) => {
                Ok(Self::execute_config_change(view, config_message))
            }            
        }
    }

    // TODO use Iterator to avoid memory allocations?
    fn create_patch(&self,
                    height: u64,
                    round: u32,
                    time: Timespec,
                    txs: &[(Hash, AnyTx<Self::Transaction>)])
                    -> Result<(Hash, Vec<Hash>, Patch), Error> {
        // Get last hash
        let last_hash = self.last_hash()?;
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

    fn execute_config_change(view: &Self::View, config_message: &ConfigMessage) {
        match *config_message {
            ConfigMessage::ConfigPropose(ref config_propose_tx) => {
                Self::handle_config_propose(view, config_propose_tx);
            }
            ConfigMessage::ConfigVote(ref config_vote_tx) => {
                Self::handle_config_vote(view, config_vote_tx);
            }
        }
    }

    fn get_height(view: &Self::View) -> u64 {
        if let Ok(Some(last_block)) = view.last_block() {
            return last_block.height() + 1;
        }
        0
    }

    fn get_actual_configuration(view: &Self::View) -> StoredConfiguration {
        let h = Self::get_height(view);
        let heights = view.configs_heights();
        let height_values = heights.values().unwrap();

        // TODO improve perfomance
        if let Some(idx) = height_values.into_iter()
            .rposition(|r| u64::from(r) <= h) {
            if let Ok(Some(height)) = heights.get(idx as u64) {
                return Self::get_configuration_at_height(view, height.into()).unwrap();
            }
        }
        unreachable!("An attempt to use exonum without genesis block");
    }

    fn commit_actual_configuration(view: &Self::View,
                                   config_propose: ConfigPropose)
                                   -> Result<(), Error> {
        let height_bytecode = config_propose.actual_from_height().into();
        view.configs().put(&height_bytecode, config_propose.config().to_vec())?;
        view.configs_heights().append(height_bytecode)?;
        // TODO: clear storages

        Ok(())
    }

    fn get_configuration_at_height(view: &Self::View, height: u64) -> Option<StoredConfiguration> {
        let configs = view.configs();
        if let Ok(Some(config)) = configs.get(&height.into()) {
            match StoredConfiguration::deserialize(&config) {
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

    fn handle_config_propose(view: &Self::View, config_propose: &ConfigPropose) {
        // Special case for genesis block with zero height
        if view.heights().len().unwrap() == 0 {
            // FIXME add is_empty method
            let hash = <ConfigPropose as Message>::hash(config_propose);
            trace!("Create initial config");
            view.config_proposes().put(&hash, config_propose.clone()).unwrap();
            Self::commit_actual_configuration(view, config_propose.clone()).unwrap();
            return;
        }

        {
            let config = Self::get_actual_configuration(view);
            if !config.validators.contains(config_propose.from()) {
                error!("ConfigPropose from unknown validator: {:?}",
                       config_propose.from());
                return;
            }

            let hash = <ConfigPropose as Message>::hash(config_propose);
            if view.config_proposes().get(&hash).unwrap().is_some() {
                error!("Received config_propose has already been handled, msg={:?}",
                       config_propose);
                return;
            }

            trace!("Handle ConfigPropose");
            view.config_proposes().put(&hash, config_propose.clone()).unwrap();
        }
    }

    fn handle_config_vote(view: &Self::View, config_vote: &ConfigVote) {
        let config = Self::get_actual_configuration(view);

        if !config.validators.contains(config_vote.from()) {
            error!("ConfigVote from unknown validator: {:?}",
                   config_vote.from());
            return;
        }

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

        if votes_count > 2 / 3 * config.validators.len() {
            if let Some(config_propose) =
                view.config_proposes()
                    .get(config_vote.hash_propose())
                    .unwrap() {
                Self::commit_actual_configuration(view, config_propose).unwrap();
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

    // FIXME make it private and find more clear name
    fn create_genesis_block(&self, cfg: GenesisConfig) -> Result<(), Error> {
        if let Some(_) = self.view().heights().get(0)? {
            // TODO create genesis block for MemoryDB and compare in hash with zero block
            // panic!("Genesis block is already created");
            return Ok(());
        }

        let genesis_block: GenesisBlock<Self> = cfg.into();
        let patch = self.create_patch(0, 0, genesis_block.time, &genesis_block.txs)?;
        self.merge(&patch.2)?;
        Ok(())
    }

    fn view(&self) -> Self::View {
        Self::View::from_fork(self.borrow().fork())
    }

    fn merge(&self, patch: &Patch) -> Result<(), Error> {
        self.deref().merge(patch)
    }
}
