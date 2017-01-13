#[macro_use]
mod spec;
mod block;
mod schema;
pub mod config;
mod genesis;
mod service;

use std::sync::Arc;

use time::Timespec;
use vec_map::VecMap;

use ::crypto::{Hash, hash};
use ::messages::{RawMessage, Precommit};

use ::storage::{Patch, Database, Fork, Error, Map, List, Storage, View as StorageView};

pub use self::block::Block;
pub use self::schema::{ConfigurationData, Schema};
pub use self::genesis::GenesisConfig;
pub use self::config::{StoredConfiguration, ConsensusConfig};
pub use self::service::{Service, Transaction};

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
        let config_propose = StoredConfiguration {
            actual_from: 0,
            validators: cfg.validators,
            consensus: cfg.consensus,
        };
        let time = Timespec {
            sec: cfg.time as i64,
            nsec: 0,
        };

        let patch = {
            let view = self.view();
            // Update service tables
            for service in self.service_map.values() {
                service.handle_genesis_block(&view)?;
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
            self.create_patch(0, 0, time, &[])?.2
        };
        self.merge(&patch)?;
        Ok(())
    }

    // TODO use Iterator to avoid memory allocations?
    pub fn create_patch(&self,
                        height: u64,
                        round: u32,
                        time: Timespec,
                        txs: &[(Hash, Box<Transaction>)])
                        -> Result<(Hash, Vec<Hash>, Patch), Error> {
        // Create fork
        let fork = self.view();
        // Create databa schema
        let schema = Schema::new(&fork);
        // Get last hash
        let last_hash = self.last_hash()?;
        // Save & execute transactions
        let mut tx_hashes = Vec::new();
        for &(hash, ref tx) in txs {
            tx.execute(&fork)?;
            schema.transactions()
                .put(&hash, tx.raw().clone())
                .unwrap();
            schema.block_txs(height)
                .append(hash)
                .unwrap();
            tx_hashes.push(hash);
        }
        // Get tx hash
        let tx_hash = schema.block_txs(height).root_hash()?;
        // Get state hash
        let state_hash = {
            // TODO Implement me with merkle table
            let mut buf = Vec::new();
            // Add core configs hashes
            buf.extend_from_slice(schema.configs().root_hash()?.as_ref());
            // Add state hashes from extensions
            for service in self.service_map.values() {
                let hash = service.state_hash(&fork)?;
                buf.extend_from_slice(hash.as_ref());
            }
            hash(&buf)
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
        Ok((block_hash, tx_hashes, fork.changes()))
    }

    pub fn commit<'a, I>(&self,
                         block_hash: Hash,
                         patch: &Patch,
                         precommits: I)
                         -> Result<Vec<Box<Transaction>>, Error>
        where I: Iterator<Item = &'a Precommit>
    {
        let (patch, txs) = {
            let view = self.db.fork();
            view.merge(patch);

            let schema = Schema::new(&view);
            for precommit in precommits {
                schema.precommits(&block_hash).append(precommit.clone())?;
            }

            // create special txs like anchoring or fee
            let mut txs = Vec::new();
            for service in self.service_map.values() {
                let t = service.handle_commit(&view)?;
                txs.extend_from_slice(&t);
            }

            (view.changes(), txs)
        };
        self.merge(&patch)?;
        Ok(txs)
    }
}