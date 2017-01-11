#[macro_use]
mod spec;
mod block;
mod schema;
pub mod config;
mod genesis;
mod service;

use std::sync::Arc;
use std::collections::HashMap;

use time::Timespec;
use vec_map::VecMap;

use ::crypto::Hash;
use ::messages::{RawMessage, Precommit};
use ::node::State;

use ::storage::{MerkleTable, MemoryDB, Patch, Database, Fork, Error, Map, List, Storage,
                View as StorageView};

pub use self::block::Block;
pub use self::schema::{ConfigurationData, Schema};
pub use self::genesis::GenesisConfig;
pub use self::config::{StoredConfiguration, ConsensusConfig};
pub use self::service::{Service, Transaction, NodeState};

#[derive(Clone)]
pub struct Blockchain {
    db: Storage,
    service_map: Arc<VecMap<Box<Service>>>,
}

impl Blockchain {
    pub fn new(db: Storage, services: Vec<Box<Service>>) -> Blockchain {
        let mut service_map = VecMap::new();
        for service in services {
            let id = service.service_id() as usize;
            if service_map.contains_key(id) {
                panic!("Services have already contain service with id={}, please change it.",
                       id);
            }
            service_map.insert(id, service);
        }

        Blockchain {
            db: db,
            service_map: Arc::new(service_map),
        }
    }

    pub fn view(&self) -> StorageView {
        self.db.fork()
    }

    pub fn tx_from_raw(&self, raw: RawMessage) -> Option<Box<Transaction>> {
        let id = raw.service_id() as usize;
        self.service_map.get(id).and_then(|service| service.tx_from_raw(raw).ok())
    }

    pub fn merge(&self, patch: &Patch) -> Result<(), Error> {
        self.db.merge(patch)
    }

    pub fn last_hash(&self) -> Result<Hash, Error> {
        Ok(Schema::new(&self.view())
            .heights()
            .last()?
            .unwrap_or_else(Hash::default))
    }

    pub fn last_block(&self) -> Result<Block, Error> {
        Ok(Schema::new(&self.view()).last_block()?.unwrap())
    }

    pub fn create_genesis_block(&self, cfg: GenesisConfig) -> Result<(), Error> {
        let mut config_propose = StoredConfiguration {
            actual_from: 0,
            validators: cfg.validators,
            consensus: cfg.consensus,
            services: HashMap::new(),
        };
        let time = Timespec {
            sec: cfg.time as i64,
            nsec: 0,
        };

        let patch = {
            let view = self.view();
            // Update service tables
            for (id, service) in self.service_map.iter() {
                let cfg = service.handle_genesis_block(&view)?;
                config_propose.services.insert(id as u16, cfg);
            }
            // Commit actual configuration
            {
                let schema = Schema::new(&view);
                if let Some(block_hash) = schema.heights().get(0)? {
                    // TODO create genesis block for MemoryDB and compare in hash with zero block
                    // panic!("Genesis block is already created");
                    let _ = block_hash;
                    return Ok(());
                }
                schema.commit_actual_configuration(0, config_propose.serialize().as_ref())?;
            };
            self.merge(&view.changes())?;
            self.create_patch(0, 0, time, &[], &HashMap::new())?.1
        };
        self.merge(&patch)?;
        Ok(())
    }

    pub fn create_patch(&self,
                        height: u64,
                        round: u32,
                        time: Timespec,
                        tx_hashes: &[Hash],
                        pool: &HashMap<Hash, Box<Transaction>>)
                        -> Result<(Hash, Patch), Error> {
        // Create fork
        let fork = self.view();
        // Create databa schema
        let schema = Schema::new(&fork);
        // Get last hash
        let last_hash = self.last_hash()?;
        // Save & execute transactions
        for hash in tx_hashes {
            let tx = &pool[hash];
            tx.execute(&fork)?;
            schema.transactions()
                .put(hash, tx.raw().clone())
                .unwrap();
            schema.block_txs(height)
                .append(*hash)
                .unwrap();
        }
        // Get tx hash
        let tx_hash = schema.block_txs(height).root_hash()?;
        // Get state hash
        let state_hash = {
            let db = MemoryDB::new();
            let hashes: MerkleTable<MemoryDB, u64, Hash> = MerkleTable::new(db);

            // Add core state hashes
            hashes.append(schema.state_hash()?)?;
            // Add state hashes from extensions
            for service in self.service_map.values() {
                if let Some(hash) = service.state_hash(&fork) {
                    hashes.append(hash?)?;
                }
            }
            hashes.root_hash()?
        };

        // Create block
        let block = Block::new(height, round, time, &last_hash, &tx_hash, &state_hash);
        // Eval block hash
        let block_hash = block.hash();
        // Update height
        // TODO: check that height == propose.height
        schema.heights().append(block_hash).is_ok();
        // Save block
        schema.blocks().put(&block_hash, block).is_ok();
        Ok((block_hash, fork.changes()))
    }

    fn execute_config_change(view: &Self::View, config_message: &ConfigMessage) {
        match *config_message {
            ConfigMessage::ConfigPropose(ref config_propose_tx) => {
                if !Self::handle_config_propose(view, config_propose_tx).is_ok() {
                    error!("ConfigPropose failed !");
                }
            }
            ConfigMessage::ConfigVote(ref config_vote_tx) => {
                if !Self::handle_config_vote(view, config_vote_tx).is_ok() {
                    error!("ConfigVote failed !");
                }
            }
        }
    }

    fn get_height(view: &Self::View) -> u64 {
        if let Ok(Some(last_block)) = view.last_block() {
            return last_block.height() + 1;
        }
        0
    }

    fn get_actual_configuration(view: &Self::View) -> Result<StoredConfiguration, Error> {
        let h = Self::get_height(view);
        let heights = view.configs_heights();
        let height_values = heights.values().unwrap();

        // TODO improve perfomance
        let idx = height_values.into_iter()
            .rposition(|r| u64::from(r) <= h)
            .unwrap();

        let height = heights.get(idx as u64)?.unwrap();
        Self::get_configuration_at_height(view, height.into()).map(|x| x.unwrap())
    }

    fn commit_actual_configuration(view: &Self::View,
                                   actual_from: u64,
                                   config_data: &[u8])
                                   -> Result<(), Error> {
        let height_bytecode = actual_from.into();
        view.configs().put(&height_bytecode, config_data.to_vec())?;
        view.configs_heights().append(height_bytecode)?;
        Ok(())
    }

    // FIXME Replace by result?
    fn get_configuration_at_height(view: &Self::View,
                                   height: u64)
                                   -> Result<Option<StoredConfiguration>, Error> {
        let configs = view.configs();
        if let Some(config) = configs.get(&height.into())? {
            match StoredConfiguration::deserialize(&config) {
                Ok(configuration) => {
                    return Ok(Some(configuration));
                }
                Err(_) => {
                    error!("Can't parse found configuration at height: {}", height);
                }
            }
        }
        Ok(None)
    }

    fn handle_config_propose(view: &Self::View,
                             config_propose: &ConfigPropose)
                             -> Result<Hash, Error> {
        let config = Self::get_actual_configuration(view).unwrap();
        if !config.validators.contains(config_propose.from()) {
            error!("ConfigPropose from unknown validator: {:?}",
                   config_propose.from());
            return Err(Error::new("ConfigPropose from unknown validator"));
        }

        let hash = <ConfigPropose as Message>::hash(config_propose);
        if view.config_proposes().get(&hash).unwrap().is_some() {
            error!("Received config_propose has already been handled, msg={:?}",
                   config_propose);
            return Err(Error::new("Received config_propose has already been handled"));
        }

        trace!("Handle ConfigPropose");
        view.config_proposes().put(&hash, config_propose.clone()).unwrap();
        Ok(hash)
    }

    fn handle_config_vote(view: &Self::View, config_vote: &ConfigVote) -> Result<(), Error> {
        let config = Self::get_actual_configuration(view).unwrap();

        if !config.validators.contains(config_vote.from()) {
            error!("ConfigVote from unknown validator: {:?}",
                   config_vote.from());
            return Err(Error::new("ConfigVote from unknown validator"));
        }

        if view.config_proposes().get(config_vote.hash_propose()).unwrap().is_none() {
            error!("Received ConfigVote for unknown transaciton, msg={:?}",
                   config_vote);
            return Err(Error::new("Received ConfigVote for unknown transaciton"));
        }

        if let Some(vote) = view.config_votes().get(config_vote.from()).unwrap() {
            if vote.seed() != config_vote.seed() - 1 {
                error!("Received config_vote with wrong seed, msg={:?}",
                       config_vote);
                return Err(Error::new("Received config_vote with wrong seed"));
            }
        }

        let msg = config_vote.clone();
        let _ = view.config_votes().put(msg.from(), config_vote.clone());

        let mut votes_count = 0;
        for pub_key in config.validators.clone() {
            if let Some(vote) = view.config_votes().get(&pub_key).unwrap() {
                if config_vote.hash_propose() == vote.hash_propose() && !vote.revoke() {
                    votes_count += 1;
                }
            }
        }

        if votes_count > 2 / 3 * config.validators.len() {
            if let Some(config_propose) =
                view.config_proposes()
                    .get(config_vote.hash_propose())
                    .unwrap() {
                Self::commit_actual_configuration(view,
                                                  config_propose.actual_from_height(),
                                                  config_propose.config())
                    .unwrap();
            }
        }
        Ok(())
    }

    pub fn commit<'a, I>(&self,
                         state: &mut State,
                         block_hash: Hash,
                         precommits: I)
                         -> Result<Vec<Box<Transaction>>, Error>
        where I: Iterator<Item = &'a Precommit>
    {
        let (patch, txs) = {
            let view = {
                let patch = state.block(&block_hash).unwrap().patch();
                let view = self.db.fork();
                view.merge(patch);
                view
            };

            let schema = Schema::new(&view);
            for precommit in precommits {
                schema.precommits(&block_hash).append(precommit.clone())?;
            }

            let mut node_state = NodeState::new(state, &view);
            for service in self.service_map.values() {
                service.handle_commit(&mut node_state)?;
            }
            (view.changes(), node_state.transactions())
        };
        self.merge(&patch)?;
        Ok(txs)
    }
}