#[macro_use] mod spec;
#[cfg(test)] mod tests;

mod raw;
mod error;
mod fields;
mod protocol;

pub use self::raw::{RawMessage, MessageBuffer, Message, HEADER_SIZE};
pub use self::error::{Error};
pub use self::fields::{Field};
pub use self::protocol::*;

pub enum Any {
    Basic(BasicMessage),
    Consensus(ConsensusMessage),
    Tx(TxMessage),
}

pub enum BasicMessage {
    Connect(Connect),
}

pub enum ConsensusMessage {
    Propose(Propose),
    Prevote(Prevote),
    Precommit(Precommit),
    Commit(Commit),
}

pub enum TxMessage {
    Issue(TxIssue),
    Transfer(TxTransfer),
    VoteValidator(TxVoteValidator),
    VoteConfig(TxVoteConfig),
}

impl Any {
    pub fn from_raw(raw: RawMessage) -> Result<Any, Error> {
        // TODO: check input message size
        Ok(match raw.message_type() {
            Connect::MESSAGE_TYPE => Any::Basic(BasicMessage::Connect(Connect::from_raw(raw)?)),
            Propose::MESSAGE_TYPE => Any::Consensus(ConsensusMessage::Propose(Propose::from_raw(raw)?)),
            Prevote::MESSAGE_TYPE => Any::Consensus(ConsensusMessage::Prevote(Prevote::from_raw(raw)?)),
            Precommit::MESSAGE_TYPE => Any::Consensus(ConsensusMessage::Precommit(Precommit::from_raw(raw)?)),
            Commit::MESSAGE_TYPE => Any::Consensus(ConsensusMessage::Commit(Commit::from_raw(raw)?)),
            _ => {
                // TODO: use result here
                panic!("unrecognized message type");
            }
        })
    }
}

