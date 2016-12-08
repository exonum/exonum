use super::super::crypto::{Hash, PublicKey};
use messages::{RawMessage, Message, Error as MessageError};

pub const CONFIG_PROPOSE_MESSAGE_ID: u16 = 12;
pub const CONFIG_VOTE_MESSAGE_ID: u16 = 13;

message !{
    TxConfigPropose{
        const ID = CONFIG_PROPOSE_MESSAGE_ID;
        const SIZE = 56;
        from:           &PublicKey  [00 => 32]
        height:         u64         [32 => 40]
        config:        &[u8]        [40 => 48] // serialized config bytes
        actual_from_height: u64     [48 => 56] // с какой высоты становится актуальным
    }
}

message !{
    TxConfigVote {
        const ID = CONFIG_VOTE_MESSAGE_ID;
        const SIZE = 81;
        from:           &PublicKey  [00 => 32]
        height:         u64         [32 => 40]
        hash_propose:   &Hash       [40 => 72] // hash of transacion we're voting for
        seed:           u64         [72 => 80] // incremental (1, 2, 3, 4, 5, 6, 7) проверять +1
        revoke:         bool        [80 => 81] // голос_за=false / отозвать=true
    }
}

#[derive(PartialEq, Debug, Clone)]
pub enum ConfigTx {
    ConfigPropose(TxConfigPropose),
    ConfigVote(TxConfigVote),    
}

impl ConfigTx {
    pub fn pub_key(&self) -> &PublicKey {
        match *self {
            ConfigTx::ConfigPropose(ref msg) => msg.from(),
            ConfigTx::ConfigVote(ref msg) => msg.from(),        
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
    fn from_raw(raw: RawMessage) -> Result<Self, MessageError> {
        Ok(match raw.message_type() {
            CONFIG_PROPOSE_MESSAGE_ID => ConfigTx::ConfigPropose(TxConfigPropose::from_raw(raw)?),
            CONFIG_VOTE_MESSAGE_ID => ConfigTx::ConfigVote(TxConfigVote::from_raw(raw)?),            
            _ => panic!("Undefined message type"),
        })
    }

    fn hash(&self) -> Hash {
        match *self {
            ConfigTx::ConfigPropose(ref msg) => msg.hash(),
            ConfigTx::ConfigVote(ref msg) => msg.hash(),            
        }
    }

    fn verify(&self, pub_key: &PublicKey) -> bool {
        match *self {
            ConfigTx::ConfigPropose(ref msg) => msg.verify(pub_key),
            ConfigTx::ConfigVote(ref msg) => msg.verify(pub_key),            
        }
    }
}
