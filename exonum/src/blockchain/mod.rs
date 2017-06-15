use vec_map::VecMap;
use byteorder::{ByteOrder, LittleEndian};
use mount::Mount;

use std::sync::Arc;
use std::collections::BTreeMap;
use std::mem;
use std::fmt;

use crypto::{self, Hash};
use messages::{RawMessage, Precommit, CONSENSUS as CORE_SERVICE};
use node::{State, TxPool};
use storage::{Patch, Database, Fork, Error, Map, List, Storage, View as StorageView};

pub use self::block::{Block, SCHEMA_MAJOR_VERSION};
pub use self::schema::{Schema, TxLocation, gen_prefix};
pub use self::genesis::GenesisConfig;
pub use self::config::{StoredConfiguration, ConsensusConfig};
pub use self::service::{Service, Transaction, ServiceContext, ApiContext};

mod block;
mod schema;
mod genesis;
mod service;
#[cfg(test)]
mod tests;

pub mod config;

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
        self.service_map
            .get(id)
            .and_then(|service| service.tx_from_raw(raw).ok())
    }

    pub fn merge(&self, patch: &Patch) -> Result<(), Error> {
        self.db.merge(patch)
    }

    pub fn last_hash(&self) -> Result<Hash, Error> {
        Ok(Schema::new(&self.view())
               .block_hashes_by_height()
               .last()?
               .unwrap_or_else(Hash::default))
    }

    pub fn last_block(&self) -> Result<Block, Error> {
        Ok(Schema::new(&self.view()).last_block()?.unwrap())
    }

    pub fn create_genesis_block(&self, cfg: GenesisConfig) -> Result<(), Error> {
        let mut config_propose = StoredConfiguration {
            previous_cfg_hash: Hash::zero(),
            actual_from: 0,
            validators: cfg.validators,
            consensus: cfg.consensus,
            services: BTreeMap::new(),
        };

        let patch = {
            let view = self.view();
            // Update service tables
            for (_, service) in self.service_map.iter() {
                let cfg = service.handle_genesis_block(&view)?;
                let name = service.service_name();
                if config_propose.services.contains_key(name) {
                    panic!("Services have already contain service with name={}, please change it.",
                           name);
                }
                config_propose.services.insert(name.into(), cfg);
            }
            // Commit actual configuration
            {
                let schema = Schema::new(&view);
                if schema.block_hash_by_height(0)?.is_some() {
                    // TODO create genesis block for MemoryDB and compare in hash with zero block
                    return Ok(());
                }
                schema.commit_configuration(config_propose)?;
            };
            self.merge(&view.changes())?;
            self.create_patch(0, 0, &[], &BTreeMap::new())?.1
        };
        self.merge(&patch)?;
        Ok(())
    }

    pub fn service_table_unique_key(service_id: u16, table_idx: usize) -> Hash {
        debug_assert!(table_idx <= u16::max_value() as usize);
        let size = mem::size_of::<u16>();
        let mut vec = vec![0; 2 * size];
        LittleEndian::write_u16(&mut vec[0..size], service_id);
        LittleEndian::write_u16(&mut vec[size..2 * size], table_idx as u16);
        crypto::hash(&vec)
    }

    pub fn create_patch(&self,
                        proposer_id: u16,
                        height: u64,
                        tx_hashes: &[Hash],
                        pool: &TxPool)
                        -> Result<(Hash, Patch), Error> {
        // Create fork
        let fork = self.view();
        // Create database schema
        let schema = Schema::new(&fork);
        // Get last hash
        let last_hash = self.last_hash()?;
        // Save & execute transactions
        for (index, hash) in tx_hashes.iter().enumerate() {
            let tx = &pool[hash];
            tx.execute(&fork)?;
            schema.transactions().put(hash, tx.raw().clone()).unwrap();
            schema.block_txs(height).append(*hash).unwrap();
            let location = TxLocation::new(height, index as u64);
            schema.tx_location_by_tx_hash().put(hash, location).unwrap();
        }
        // Get tx hash
        let tx_hash = schema.block_txs(height).root_hash()?;
        // Get tx count
        let tx_count = schema.block_txs(height).len()? as u32;
        // Get state hash
        let state_hash = {
            let sum_table = schema.state_hash_aggregator();
            let vec_core_state = schema.core_state_hash()?;
            for (idx, core_table_hash) in vec_core_state.into_iter().enumerate() {
                let key = Blockchain::service_table_unique_key(CORE_SERVICE, idx);
                sum_table.put(&key, core_table_hash)?;
            }
            for service in self.service_map.values() {
                let service_id = service.service_id();
                let vec_service_state = service.state_hash(&fork)?;
                for (idx, service_table_hash) in vec_service_state.into_iter().enumerate() {
                    let key = Blockchain::service_table_unique_key(service_id, idx);
                    sum_table.put(&key, service_table_hash)?;
                }
            }
            sum_table.root_hash()?
        };

        // Create block
        let block = Block::new(SCHEMA_MAJOR_VERSION,
                               proposer_id,
                               height,
                               tx_count,
                               &last_hash,
                               &tx_hash,
                               &state_hash);
        trace!("execute block = {:?}", block);
        // Eval block hash
        let block_hash = block.hash();
        // Update height
        // TODO: check that height == propose.height
        schema.block_hashes_by_height().append(block_hash).is_ok();
        // Save block
        schema.blocks().put(&block_hash, block).is_ok();
        Ok((block_hash, fork.changes()))
    }

    #[cfg_attr(feature="flame_profile", flame)]
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

            state.update_config(schema.actual_configuration()?);

            let mut ctx = ServiceContext::new(state, &view);
            for service in self.service_map.values() {
                service.handle_commit(&mut ctx)?;
            }
            (view.changes(), ctx.transactions())
        };
        self.merge(&patch)?;
        Ok(txs)
    }

    pub fn mount_public_api(&self, context: &ApiContext) -> Mount {
        let mut mount = Mount::new();
        for service in self.service_map.values() {
            if let Some(handler) = service.public_api_handler(context) {
                mount.mount(service.service_name(), handler);
            }
        }
        mount
    }

    pub fn mount_private_api(&self, context: &ApiContext) -> Mount {
        let mut mount = Mount::new();
        for service in self.service_map.values() {
            if let Some(handler) = service.private_api_handler(context) {
                mount.mount(service.service_name(), handler);
            }
        }
        mount
    }
}

impl fmt::Debug for Blockchain {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,
               "Blockchain {{ db: {:?}, service_map: {{ .. }} }}",
               self.db)
    }
}
