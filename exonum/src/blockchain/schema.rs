use byteorder::{ByteOrder, BigEndian};

use std::mem;

use crypto::Hash;
use messages::{RawMessage, Precommit, BlockProof, CONSENSUS};
use storage::{StorageValue, ListTable, MapTable, MerkleTable, MerklePatriciaTable, Error, Map,
              List, RootProofNode, View};
use super::{Block, Blockchain};
use super::config::StoredConfiguration;

/// Generates prefix that combines service identifier,
/// table identifier and given suffix.
pub fn gen_prefix(service_id: u16, ord: u8, suf: Option<&[u8]>) -> Vec<u8> {
    let pos = mem::size_of::<u16>();
    let mut res;
    if let Some(suffix) = suf {
        let size = pos + 1 + suffix.len();
        res = vec![0; size];
        res[pos + 1..].copy_from_slice(suffix);
    } else {
        res = vec![0; pos + 1];
    }
    BigEndian::write_u16(&mut res[0..pos], service_id);
    res[pos] = ord;
    res
}

storage_value! (
    /// Configuration index.
    struct ConfigReference {
        const SIZE = 40;
        /// The height, starting from which this configuration becomes actual.
        field actual_from: u64    [00 => 08]
        /// Hash of the configuration contents that serialized as raw bytes vec.
        field cfg_hash:    &Hash  [08 => 40]
    }
);

storage_value! (
    /// Transaction location in block.
    struct TxLocation {
        const SIZE = 16;
        /// Height of block in the blockchain.
        field block_height:         u64  [00 => 08]
        /// Index in block.
        field position_in_block:    u64  [08 => 16]
    }
);

/// Information schema for `exonum-core`.
#[derive(Debug)]
pub struct Schema<'a> {
    view: &'a View,
}

/// Data tables section.
impl<'a> Schema<'a> {
    /// Returns table that represents a map from transaction hash into raw transaction message.
    pub fn transactions(&self) -> MapTable<View, Hash, RawMessage> {
        MapTable::new(gen_prefix(CONSENSUS, 0, None), self.view)
    }

    /// Returns table that keeps the block height and tx position inside block for every
    /// transaction hash.
    pub fn tx_location_by_tx_hash(&self) -> MapTable<View, Hash, TxLocation> {
        MapTable::new(gen_prefix(CONSENSUS, 1, None), self.view)
    }

    /// Returns table that stores block object for every block height.
    pub fn blocks(&self) -> MapTable<View, Hash, Block> {
        MapTable::new(gen_prefix(CONSENSUS, 2, None), self.view)
    }

    /// Returns table that keeps block hash for the corresponding height.
    pub fn block_hashes_by_height(&self) -> ListTable<MapTable<View, [u8], Vec<u8>>, Hash> {
        ListTable::new(MapTable::new(gen_prefix(CONSENSUS, 3, None), self.view))
    }

    /// Returns table that keeps a list of transactions for the each block.
    pub fn block_txs(&self, height: u64) -> MerkleTable<MapTable<View, [u8], Vec<u8>>, Hash> {
        MerkleTable::new(MapTable::new(gen_prefix(CONSENSUS, 4, Some(&height.serialize())),
                                       self.view))
    }

    /// Returns table that saves a list of precommits for block with given hash.
    pub fn precommits(&self, hash: &Hash) -> ListTable<MapTable<View, [u8], Vec<u8>>, Precommit> {
        ListTable::new(MapTable::new(gen_prefix(CONSENSUS, 5, Some(hash.as_ref())), self.view))
    }

    /// Returns table that represents a map from configuration hash into contents.
    pub fn configs
        (&self)
         -> MerklePatriciaTable<MapTable<View, [u8], Vec<u8>>, Hash, StoredConfiguration> {
        // configs patricia merkletree <block height> json
        MerklePatriciaTable::new(MapTable::new(gen_prefix(CONSENSUS, 6, None), self.view))
    }

    /// Returns auxiliary table that keeps hash references to configurations in order
    /// of increasing their `actual_from` height.
    pub fn configs_actual_from(&self) -> ListTable<MapTable<View, [u8], Vec<u8>>, ConfigReference> {
        ListTable::new(MapTable::new(gen_prefix(CONSENSUS, 7, None), self.view))
    }

    /// Returns the accessory `MerklePatriciaTable` for calculating
    /// patches in the DBView layer.
    ///
    /// Table calculates "aggregation" of root hashes of individual
    /// service tables, in effect summing the state of various entities,
    /// scattered across distinct services and their tables. Sum is performed by
    /// means of computing root hash of this table.
    ///
    /// - Table **key** is 32 bytes of normalized coordinates of a service
    /// table, as returned by `service_table_unique_key` helper function.
    /// - Table **value** is root hash of a service table, which contributes
    /// to the resulting block's `state_hash`.
    ///
    /// Core tables participate in resulting state_hash with `CORE_SERVICE`
    /// service_id. Their vector is returned by `core_state_hash` method.
    pub fn state_hash_aggregator
        (&self)
         -> MerklePatriciaTable<MapTable<View, [u8], Vec<u8>>, Hash, Hash> {
        MerklePatriciaTable::new(MapTable::new(gen_prefix(CONSENSUS, 8, None), self.view))
    }
}

/// Business logic section.
impl<'a> Schema<'a> {
    /// Constructs schema for the given storage `View`.
    pub fn new(view: &'a View) -> Schema {
        Schema { view: view }
    }

    /// Returns block hash for the given height.
    pub fn block_hash_by_height(&self, height: u64) -> Result<Option<Hash>, Error> {
        self.block_hashes_by_height().get(height)
    }

    /// Returns the block for the given height with the proof of its inclusion.
    pub fn block_and_precommits(&self, height: u64) -> Result<Option<BlockProof>, Error> {
        let block_hash = match self.block_hash_by_height(height)? {
            None => return Ok(None),
            Some(block_hash) => block_hash,
        };
        let block = self.blocks().get(&block_hash)?.unwrap();
        let precommits_table = self.precommits(&block_hash);
        let precommits = precommits_table.values()?;
        let res = BlockProof {
            block: block,
            precommits: precommits,
        };
        Ok(Some(res))
    }

    /// Returns latest committed block.
    pub fn last_block(&self) -> Result<Option<Block>, Error> {
        Ok(match self.block_hashes_by_height().last()? {
               Some(hash) => Some(self.blocks().get(&hash)?.unwrap()),
               None => None,
           })
    }

    /// Returns height of the latest committed block.
    pub fn last_height(&self) -> Result<Option<u64>, Error> {
        let block_opt = self.last_block()?;
        Ok(block_opt.map(|block| block.height()))
    }

    /// Returns the current height of the blockchain. Its value is equal to `last_height + 1`.
    pub fn current_height(&self) -> Result<u64, Error> {
        let last_height = self.last_height()?;
        let res = match last_height {
            Some(last_height) => last_height + 1,
            None => 0,
        };
        Ok(res)
    }

    /// Adds a new configuration to the blockchain, which will become an actual at
    /// the `actual_from` height in `config_data`.
    pub fn commit_configuration(&self, config_data: StoredConfiguration) -> Result<(), Error> {
        let actual_from = config_data.actual_from;
        if let Some(last_cfg_reference) = self.configs_actual_from().last()? {
            let last_actual_from = last_cfg_reference.actual_from();
            if actual_from <= last_actual_from {
                return Err(Error::new(format!("Attempting to commit configuration \
                                               with actual_from {:?} less than \
                                              the last committed actual_from {:?}",
                                              actual_from,
                                              last_actual_from)));
            }
        }
        let cfg_hash = config_data.hash();
        self.configs().put(&cfg_hash, config_data.clone())?;

        let cfg_ref = ConfigReference::new(actual_from, &cfg_hash);
        self.configs_actual_from().append(cfg_ref)?;
        info!("Scheduled the following configuration for acceptance: {:?}",
              config_data);
        // TODO: clear storages
        Ok(())
    }

    /// Returns configuration for the latest height of blockchain.
    pub fn actual_configuration(&self) -> Result<StoredConfiguration, Error> {
        let current_height = self.current_height()?;
        let res = self.configuration_by_height(current_height);
        trace!("Retrieved actual_config: {:?}", res);
        res
    }

    /// Returns the nearest following configuration if it exists.
    pub fn following_configuration(&self) -> Result<Option<StoredConfiguration>, Error> {
        let current_height = self.current_height()?;
        let idx = self.find_configurations_index_by_height(current_height)?;
        let res = match self.configs_actual_from().get(idx + 1)? {
            Some(cfg_ref) => {
                let cfg_hash = cfg_ref.cfg_hash();
                let cfg =
                    self.configuration_by_hash(cfg_hash)?
                        .expect(&format!("Config with hash {:?} is absent in configs table",
                                        cfg_hash));
                Some(cfg)
            }
            None => None,
        };
        Ok(res)
    }

    /// Returns the previous configuration if it exists.
    pub fn previous_configuration(&self) -> Result<Option<StoredConfiguration>, Error> {
        let current_height = self.current_height()?;
        let idx = self.find_configurations_index_by_height(current_height)?;
        let res = if idx > 0 {
            let cfg_ref = self.configs_actual_from()
                .get(idx - 1)?
                .expect(&format!("Configuration at index {} not found", idx));
            let cfg_hash = cfg_ref.cfg_hash();
            let cfg =
                self.configuration_by_hash(cfg_hash)?
                    .expect(&format!("Config with hash {:?} is absent in configs table", cfg_hash));
            Some(cfg)
        } else {
            None
        };
        Ok(res)
    }

    /// Returns the configuration that is the actual for the given height.
    pub fn configuration_by_height(&self, height: u64) -> Result<StoredConfiguration, Error> {
        let idx = self.find_configurations_index_by_height(height)?;
        let cfg_ref = self.configs_actual_from()
            .get(idx)?
            .expect(&format!("Configuration at index {} not found", idx));
        let cfg_hash = cfg_ref.cfg_hash();
        let cfg =
            self.configuration_by_hash(cfg_hash)?
                .expect(&format!("Config with hash {:?} is absent in configs table", cfg_hash));
        Ok(cfg)
    }

    /// Returns configuration for given configuration hash.
    pub fn configuration_by_hash(&self, hash: &Hash) -> Result<Option<StoredConfiguration>, Error> {
        self.configs().get(hash)
    }

    /// Returns the `state_hash` table for core tables.
    pub fn core_state_hash(&self) -> Result<Vec<Hash>, Error> {
        Ok(vec![self.configs().root_hash()?])
    }

    /// Constructs a proof of inclusion of root hash of a specific service
    /// table into block's `state_hash`.
    ///
    /// Searched key for proof is uniquely identified by (`u16`, `u16`) tuple
    /// of table's coordinates.
    ///
    /// If found, root hash is returned as a value of proof's leaf
    /// corresponding to searched key. Otherwise, partial path to searched key
    /// is returned, which proves its exclusion.
    ///
    /// The returned proof is used as a component of proof of state of any
    /// entity, stored in `exonum` db at specific height, as identified
    /// by corresponding block's `state_hash`. State of some meta tables
    /// of core and services isn't tracked.
    ///
    /// # Arguments
    ///
    /// * `service_id` - `service_id` as returned by instance of type of
    /// `Service` trait
    /// * `table_idx` - index of service table in `Vec`, returned by
    /// `state_hash` method of instance of type of `Service` trait
    pub fn get_proof_to_service_table(&self,
                                      service_id: u16,
                                      table_idx: usize)
                                      -> Result<RootProofNode<Hash>, Error> {
        let key = Blockchain::service_table_unique_key(service_id, table_idx);
        let sum_table = self.state_hash_aggregator();
        sum_table.construct_path_to_key(key)
    }

    fn find_configurations_index_by_height(&self, height: u64) -> Result<u64, Error> {
        let configs_actual_from = self.configs_actual_from();
        let cfg_references = configs_actual_from.values()?;

        let idx = cfg_references
            .into_iter()
            .rposition(|r| r.actual_from() <= height)
            .expect(&format!("Couldn't not find any config for height {},\
          that means that genesis block was created incorrectly.",
                            height));
        Ok(idx as u64)
    }
}
