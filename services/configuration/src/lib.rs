#[macro_use]
extern crate exonum;
#[macro_use]
extern crate log;

extern crate serde;

use std::fmt;

use serde::{Serialize, Serializer};

use exonum::blockchain::{Service, Transaction, Schema, NodeState};
use exonum::crypto::{PublicKey, Hash, HexValue};
use exonum::messages::{RawMessage, Message, FromRaw, RawTransaction, Error as MessageError};
use exonum::storage::{MerkleTable, MemoryDB, Map, List, View, MapTable, MerklePatriciaTable,
                      Result as StorageResult};

pub const CONFIG_SERVICE: u16 = 1;
pub const CONFIG_PROPOSE_MESSAGE_ID: u16 = 0;
pub const CONFIG_VOTE_MESSAGE_ID: u16 = 1;

message! {
    TxConfigPropose {
        const TYPE = CONFIG_SERVICE;        
        const ID = CONFIG_PROPOSE_MESSAGE_ID;
        const SIZE = 56; 

        from:           &PublicKey  [00 => 32]
        height:         u64         [32 => 40]
        config:        &[u8]        [40 => 48] // serialized config bytes
        actual_from_height: u64     [48 => 56] // с какой высоты становится актуальным
    }
}

message! {
    TxConfigVote {
        const TYPE = CONFIG_SERVICE;   
        const ID = CONFIG_VOTE_MESSAGE_ID;
        const SIZE = 81; 
        
        from:           &PublicKey  [00 => 32]
        height:         u64         [32 => 40]
        hash_propose:   &Hash       [40 => 72] // hash of transacion we're voting for
        seed:           u64         [72 => 80] // incremental (1, 2, 3, 4, 5, 6, 7) проверять +1
        revoke:         bool        [80 => 81] // голос_за=false / отозвать=true
    }
}

#[derive(Clone, PartialEq)]
pub enum ConfigTx {
    ConfigPropose(TxConfigPropose),
    ConfigVote(TxConfigVote),
}

impl Serialize for TxConfigPropose {
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        let mut state;
        state = ser.serialize_struct("config_propose", 4)?;
        ser.serialize_struct_elt(&mut state, "from", self.from().to_hex())?;
        ser.serialize_struct_elt(&mut state, "height", self.height())?;
        ser.serialize_struct_elt(&mut state, "config", self.config())?;
        ser.serialize_struct_elt(&mut state, "actual_from_height", self.actual_from_height())?;
        ser.serialize_struct_end(state)
    }
}

impl Serialize for TxConfigVote {
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        let mut state;
        state = ser.serialize_struct("vote", 5)?;
        ser.serialize_struct_elt(&mut state, "from", self.from().to_hex())?;
        ser.serialize_struct_elt(&mut state, "height", self.height())?;
        ser.serialize_struct_elt(&mut state, "hash_propose", self.hash_propose().to_hex())?;
        ser.serialize_struct_elt(&mut state, "seed", self.seed())?;
        ser.serialize_struct_elt(&mut state, "revoke", self.revoke())?;
        ser.serialize_struct_end(state)
    }
}

impl Serialize for ConfigTx {
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        match *self {
            ConfigTx::ConfigPropose(ref propose) => propose.serialize(ser),
            ConfigTx::ConfigVote(ref vote) => vote.serialize(ser),            
        }
    }
}

#[derive(Default)]
pub struct ConfigurationService {}

pub struct ConfigurationSchema<'a> {
    view: &'a View,
}

impl ConfigTx {
    pub fn from(&self) -> &PublicKey {
        match *self {
            ConfigTx::ConfigPropose(ref msg) => msg.from(),
            ConfigTx::ConfigVote(ref msg) => msg.from(),
        }
    }

    pub fn height(&self) -> u64 {
        match *self {
            ConfigTx::ConfigPropose(ref msg) => msg.height(),
            ConfigTx::ConfigVote(ref msg) => msg.height(),
        }
    }
}

impl fmt::Debug for ConfigTx {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match *self {
            ConfigTx::ConfigPropose(ref msg) => write!(fmt, "{:?}", msg),
            ConfigTx::ConfigVote(ref msg) => write!(fmt, "{:?}", msg),
        }
    }
}

impl FromRaw for ConfigTx {
    fn from_raw(raw: RawMessage) -> Result<ConfigTx, MessageError> {
        match raw.message_type() {
            CONFIG_PROPOSE_MESSAGE_ID => {
                Ok(ConfigTx::ConfigPropose(TxConfigPropose::from_raw(raw)?))
            }
            CONFIG_VOTE_MESSAGE_ID => Ok(ConfigTx::ConfigVote(TxConfigVote::from_raw(raw)?)),
            _ => Err(MessageError::IncorrectMessageType { message_type: raw.message_type() }),
        }
    }
}

impl Message for ConfigTx {
    fn raw(&self) -> &RawMessage {
        match *self {
            ConfigTx::ConfigPropose(ref msg) => msg.raw(),
            ConfigTx::ConfigVote(ref msg) => msg.raw(),
        }
    }

    fn verify_signature(&self, public_key: &PublicKey) -> bool {
        match *self {
            ConfigTx::ConfigPropose(ref msg) => msg.verify_signature(public_key),
            ConfigTx::ConfigVote(ref msg) => msg.verify_signature(public_key),
        }
    }

    fn hash(&self) -> Hash {
        match *self {
            ConfigTx::ConfigPropose(ref msg) => msg.hash(),
            ConfigTx::ConfigVote(ref msg) => msg.hash(),
        }
    }
}

impl<'a> ConfigurationSchema<'a> {
    pub fn new(view: &'a View) -> ConfigurationSchema {
        ConfigurationSchema { view: view }
    }

    fn config_proposes
        (&self)
         -> MerklePatriciaTable<MapTable<View, [u8], Vec<u8>>, Hash, TxConfigPropose> {
        // config_propose paricia merkletree <hash_tx> транзакция пропоз
        MerklePatriciaTable::new(MapTable::new(vec![04], self.view))
    }

    fn config_votes
        (&self)
         -> MerklePatriciaTable<MapTable<View, [u8], Vec<u8>>, PublicKey, TxConfigVote> {
        // config_votes patricia merkletree <pub_key> последний голос
        MerklePatriciaTable::new(MapTable::new(vec![05], self.view))
    }


    pub fn state_hash(&self) -> StorageResult<Vec<Hash>> {
        Ok(vec![self.config_proposes().root_hash()?, self.config_votes().root_hash()?])
    }

    pub fn get_config_propose(&self,
                              hash: &Hash)
                              -> Result<Option<TxConfigPropose>, exonum::storage::Error> {
        self.config_proposes().get(hash)
    }

    pub fn get_vote(&self,
                    pub_key: &PublicKey)
                    -> Result<Option<TxConfigVote>, exonum::storage::Error> {
        self.config_votes().get(pub_key)
    }
}

impl TxConfigPropose {
    fn execute(&self, view: &View) -> StorageResult<()> {
        let blockchain_schema = Schema::new(view);
        let config_schema = ConfigurationSchema::new(view);

        let config = blockchain_schema.get_actual_configuration()?;
        if !config.validators.contains(self.from()) {
            error!("ConfigPropose from unknown validator: {:?}", self.from());
            return Ok(());
        }

        let hash = self.hash();
        if config_schema.config_proposes().get(&hash)?.is_some() {
            error!("Received config_propose has already been handled, msg={:?}",
                   self);
            return Ok(());
        }

        trace!("Handle ConfigPropose");
        config_schema.config_proposes().put(&hash, self.clone())?;
        Ok(())
    }
}

impl TxConfigVote {
    fn execute(&self, view: &View) -> StorageResult<()> {
        let blockchain_schema = Schema::new(view);
        let config_schema = ConfigurationSchema::new(view);

        let config = blockchain_schema.get_actual_configuration()?;

        if !config.validators.contains(self.from()) {
            error!("ConfigVote from unknown validator: {:?}", self.from());
            return Ok(());
        }

        if config_schema.config_proposes().get(self.hash_propose())?.is_none() {
            error!("Received config_vote for unknown transaciton, msg={:?}",
                   self);
            return Ok(());
        }

        if let Some(vote) = config_schema.config_votes().get(self.from())? {
            if vote.seed() != self.seed() - 1 {
                error!("Received config_vote with wrong seed, msg={:?}", self);
                return Ok(());
            }
        }

        let msg = self.clone();
        config_schema.config_votes().put(msg.from(), self.clone())?;

        let mut votes_count = 0;
        for pub_key in config.validators.clone() {
            if let Some(vote) = config_schema.config_votes().get(&pub_key)? {
                if !vote.revoke() {
                    votes_count += 1;
                }
            }
        }

        if votes_count > 2 / 3 * config.validators.len() {
            if let Some(config_propose) =
                config_schema.config_proposes()
                    .get(self.hash_propose())? {
                blockchain_schema.commit_actual_configuration(config_propose.actual_from_height(),
                                                 config_propose.config())?;
            }
        }
        Ok(())
    }
}

impl Transaction for ConfigTx {
    fn verify(&self) -> bool {
        self.verify_signature(self.from())
    }

    fn execute(&self, view: &View) -> StorageResult<()> {
        match *self {
            ConfigTx::ConfigPropose(ref tx) => tx.execute(view),
            ConfigTx::ConfigVote(ref tx) => tx.execute(view),
        }
    }
}

impl ConfigurationService {
    pub fn new() -> ConfigurationService {
        ConfigurationService {}
    }
}

impl Service for ConfigurationService {
    fn service_id(&self) -> u16 {
        CONFIG_SERVICE
    }

    fn state_hash(&self, view: &View) -> StorageResult<Vec<Hash>> {
        let schema = ConfigurationSchema::new(view);
        schema.state_hash()
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, MessageError> {
        ConfigTx::from_raw(raw).map(|tx| Box::new(tx) as Box<Transaction>)
    }

    fn handle_commit(&self, state: &mut NodeState) -> StorageResult<()> {
        let actual_config = {
            let schema = Schema::new(state.view());
            let height = state.height();
            schema.get_configuration_at_height(height)?
        };

        if let Some(config) = actual_config {
            info!("Updated node config={:#?}", config);
            state.update_config(config);
        }
        Ok(())
    }
}