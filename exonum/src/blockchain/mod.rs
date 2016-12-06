extern crate serde_json;

#[macro_use]
mod spec;
mod block;
mod view;
mod config;

use std::borrow::Borrow;
use std::ops::Deref;

use time::Timespec;

use ::crypto::{Hash, hash};
use ::messages::{Precommit, Message, AnyTx, ServiceTx};
use ::storage::{StorageValue, Patch, Database, Fork, Error, Map, List};

pub use self::block::Block;
pub use self::view::View;
pub use self::view::ConfigurationData;
pub use self::view::HeightBytecode;

use ::node::StoredConfiguration;

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

    fn get_initial_configuration (&self) -> Option<StoredConfiguration> {
        let r = self.last_block().unwrap();
        let last_height = if let Some(last_block) = r {
            last_block.height() + 1
        } else {
            0
        };
        let mut h = last_height;

        while h > 0 {
            if let Some(configuration) = self.get_configuration_at_height(h){
                return Some(configuration);
            }
            h -= 1;
        }
        None
    }

    fn get_configuration_at_height (&self, height: u64) -> Option<StoredConfiguration> {
        let view = self.view();
        let configs = view.configs();
        if let Ok(config) = configs.get(&StoredConfiguration::height_to_slice(height)) {
            match StoredConfiguration::deserialize(&config.unwrap()) {
                Ok(configuration) => {
                    return Some(configuration);
                },
                Err(_) => {
                    error!("Can't parse found configuration at height: {}", height);
                }
            }
        }
        None
    }

    // fn handle_config_propose(&self, config_propose: ConfigPropose) {

    //     if config_propose.height() < self.state.height() || config_propose.height() > self.state.height() + 1 {
    //         warn!("Received ConfigPropose message from other height: msg.height={}, self.height={}",
    //               config_propose.height(),
    //               self.state.height());
    //         return;
    //     }

    //     if config_propose.actual_from_height() < self.state.height() {
    //         error!("Received config for past height: msg.actual_from_height={}, self.height={}",
    //             config_propose.actual_from_height(), self.state.height());
    //         return;
    //     }

    //     if !self.state.validators().contains(config_propose.from()){
    //         error!("ConfigPropose from unknown validator: {:?}", config_propose.from());
    //         return;
    //     }

    //     let view = self.blockchain.view();
    //     if view.config_proposes().get(&config_propose.hash()).unwrap().is_some() {
    //         error!("Received config_propose has already been handled, msg={:?}", config_propose);
    //         return;
    //     }

    //     trace!("Handle ConfigPropose");
    //     let _ = view.config_proposes().put(&config_propose.hash(), config_propose);

    // }

    // fn handle_config_vote(&self, config_vote: ConfigVote){

    //     if config_vote.height() < self.state.height() || config_vote.height() > self.state.height() + 1 {
    //         warn!("Received ConfigVote message from other height: msg.height={}, self.height={}",
    //               config_vote.height(),
    //               self.state.height());
    //         return;
    //     }

    //     if !self.state.validators().contains(config_vote.from()){
    //         error!("ConfigVote from unknown validator: {:?}", config_vote.from());
    //         return;
    //     }

    //     let view = self.blockchain.view();
    //     if view.config_proposes().get(config_vote.hash_propose()).unwrap().is_some() {
    //         error!("Received config_vote for unknown transaciton, msg={:?}", config_vote);
    //         return;
    //     }

    //     if let Some(vote) = view.config_votes().get(config_vote.from()).unwrap() {
    //         if vote.seed() != config_vote.seed() -1 {
    //             error!("Received config_vote with wrong seed, msg={:?}", config_vote);
    //             return;
    //         }
    //     }

    //     let msg = config_vote.clone();
    //     let _ = view.config_votes().put(msg.from(), config_vote.clone());

    //     let mut votes_count = 0;
    //     for pub_key in self.state.validators(){
    //         if let Some(vote) = view.config_votes().get(pub_key).unwrap() {
    //             if !vote.revoke() {
    //                 votes_count += 1;
    //             }
    //         }
    //     }

    //     if votes_count >= 2/3 * self.state.validators().len(){
    //         if let Some(config_propose) = view.config_proposes().get(config_vote.hash_propose()).unwrap() {
    //             view.configs().put(&StoredConfiguration::height_to_slice(config_propose.actual_from_height()), config_propose.config().to_vec()).unwrap();
    //             // TODO: clear storages
    //         }
    //     }
    // }

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
