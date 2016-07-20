#[macro_use] mod spec;
#[cfg(test)] mod tests;

mod raw;
mod error;
mod fields;
mod protocol;

use time::{Timespec};

use super::crypto::{Hash, PublicKey};

pub use self::raw::{RawMessage, MessageBuffer, Message, HEADER_SIZE};
pub use self::error::{Error};
pub use self::fields::{Field, SegmentField};
pub use self::protocol::*;

// TODO: implement common methods for enum types (hash, raw, from_raw, verify)
// TODO: use macro for implementing enums

#[derive(Clone)]
pub enum Any {
    Basic(BasicMessage),
    Consensus(ConsensusMessage),
    Tx(TxMessage),
}

#[derive(Clone)]
pub enum BasicMessage {
    Connect(Connect),
}

#[derive(Clone)]
pub enum ConsensusMessage {
    Propose(Propose),
    Prevote(Prevote),
    Precommit(Precommit),
    Commit(Commit),
}

#[derive(Clone)]
pub enum RequestMessage {
    Propose(RequestPropose),
    Transactions(RequestTransactions),
    Prevotes(RequestPrevotes),
    Precommits(RequestPrecommits),
    Commit(RequestCommit),
    Peers(RequestPeers)
}

#[derive(Clone)]
pub enum TxMessage {
    Issue(TxIssue),
    Transfer(TxTransfer),
    VoteValidator(TxVoteValidator),
    VoteConfig(TxVoteConfig),
}

impl TxMessage {
    pub fn hash(&self) -> Hash {
        match *self {
            TxMessage::Issue(ref msg) => msg.hash(),
            TxMessage::Transfer(ref msg) => msg.hash(),
            TxMessage::VoteValidator(ref msg) => msg.hash(),
            TxMessage::VoteConfig(ref msg) => msg.hash()
        }
    }

    pub fn raw(&self) -> &RawMessage {
        match *self {
            TxMessage::Issue(ref msg) => msg.raw(),
            TxMessage::Transfer(ref msg) => msg.raw(),
            TxMessage::VoteValidator(ref msg) => msg.raw(),
            TxMessage::VoteConfig(ref msg) => msg.raw()
        }
    }

    pub fn from_raw(raw: RawMessage) -> Result<TxMessage, Error> {
        // TODO: check input message size
        Ok(match raw.message_type() {
            TxIssue::MESSAGE_TYPE => TxMessage::Issue(TxIssue::from_raw(raw)?),
            TxTransfer::MESSAGE_TYPE => TxMessage::Transfer(TxTransfer::from_raw(raw)?),
            TxVoteValidator::MESSAGE_TYPE => TxMessage::VoteValidator(TxVoteValidator::from_raw(raw)?),
            TxVoteConfig::MESSAGE_TYPE => TxMessage::VoteConfig(TxVoteConfig::from_raw(raw)?),
            _ => {
                // TODO: use result here
                panic!("unrecognized message type");
            }
        })
    }
}

impl RequestMessage {
    pub fn from(&self) -> u32 {
        match *self {
            RequestMessage::Propose(ref msg) => msg.from(),
            RequestMessage::Transactions(ref msg) => msg.from(),
            RequestMessage::Prevotes(ref msg) => msg.from(),
            RequestMessage::Precommits(ref msg) => msg.from(),
            RequestMessage::Commit(ref msg) => msg.from(),
            RequestMessage::Peers(ref msg) => msg.from(),
        }
    }

    pub fn to(&self) -> u32 {
        match *self {
            RequestMessage::Propose(ref msg) => msg.to(),
            RequestMessage::Transactions(ref msg) => msg.to(),
            RequestMessage::Prevotes(ref msg) => msg.to(),
            RequestMessage::Precommits(ref msg) => msg.to(),
            RequestMessage::Commit(ref msg) => msg.to(),
            RequestMessage::Peers(ref msg) => msg.to(),
        }
    }

    pub fn time(&self) -> Timespec {
        match *self {
            RequestMessage::Propose(ref msg) => msg.time(),
            RequestMessage::Transactions(ref msg) => msg.time(),
            RequestMessage::Prevotes(ref msg) => msg.time(),
            RequestMessage::Precommits(ref msg) => msg.time(),
            RequestMessage::Commit(ref msg) => msg.time(),
            RequestMessage::Peers(ref msg) => msg.time(),
        }
    }

    pub fn verify(&self, public_key: &PublicKey) -> bool {
        match *self {
            RequestMessage::Propose(ref msg) => msg.verify(public_key),
            RequestMessage::Transactions(ref msg) => msg.verify(public_key),
            RequestMessage::Prevotes(ref msg) => msg.verify(public_key),
            RequestMessage::Precommits(ref msg) => msg.verify(public_key),
            RequestMessage::Commit(ref msg) => msg.verify(public_key),
            RequestMessage::Peers(ref msg) => msg.verify(public_key),
        }
    }

    pub fn raw(&self) -> &RawMessage {
        match *self {
            RequestMessage::Propose(ref msg) => msg.raw(),
            RequestMessage::Transactions(ref msg) => msg.raw(),
            RequestMessage::Prevotes(ref msg) => msg.raw(),
            RequestMessage::Precommits(ref msg) => msg.raw(),
            RequestMessage::Commit(ref msg) => msg.raw(),
            RequestMessage::Peers(ref msg) => msg.raw(),
        }
    }
}

impl ConsensusMessage {
    pub fn validator(&self) -> u32 {
        match *self {
            ConsensusMessage::Propose(ref msg) => msg.validator(),
            ConsensusMessage::Prevote(ref msg) => msg.validator(),
            ConsensusMessage::Precommit(ref msg) => msg.validator(),
            ConsensusMessage::Commit(ref msg) => msg.validator(),
        }
    }

    pub fn height(&self) -> u64 {
        match *self {
            ConsensusMessage::Propose(ref msg) => msg.height(),
            ConsensusMessage::Prevote(ref msg) => msg.height(),
            ConsensusMessage::Precommit(ref msg) => msg.height(),
            ConsensusMessage::Commit(ref msg) => msg.height(),
        }
    }

    pub fn round(&self) -> u32 {
        match *self {
            ConsensusMessage::Propose(ref msg) => msg.round(),
            ConsensusMessage::Prevote(ref msg) => msg.round(),
            ConsensusMessage::Precommit(ref msg) => msg.round(),
            ConsensusMessage::Commit(ref msg) => msg.round(),
        }
    }

    pub fn raw(&self) -> &RawMessage {
        match *self {
            ConsensusMessage::Propose(ref msg) => msg.raw(),
            ConsensusMessage::Prevote(ref msg) => msg.raw(),
            ConsensusMessage::Precommit(ref msg) => msg.raw(),
            ConsensusMessage::Commit(ref msg) => msg.raw(),
        }
    }

    pub fn verify(&self, public_key: &PublicKey) -> bool {
        match *self {
            ConsensusMessage::Propose(ref msg) => msg.verify(public_key),
            ConsensusMessage::Prevote(ref msg) => msg.verify(public_key),
            ConsensusMessage::Precommit(ref msg) => msg.verify(public_key),
            ConsensusMessage::Commit(ref msg) => msg.verify(public_key),
        }
    }
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

