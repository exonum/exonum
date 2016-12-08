#[macro_use]
mod spec;
#[cfg(test)]
mod tests;

use std::fmt;

mod raw;
mod error;
mod fields;
mod protocol;

use time::Timespec;
use bit_vec;

use ::crypto::{PublicKey, Hash};
use ::blockchain::Blockchain;

pub use self::raw::{RawMessage, MessageWriter, MessageBuffer, Message, HEADER_SIZE};
pub use self::error::Error;
pub use self::fields::{Field, SegmentField};
pub use self::protocol::*;

pub type BitVec = bit_vec::BitVec;

// TODO: implement common methods for enum types (hash, raw, from_raw, verify)
// TODO: use macro for implementing enums

#[derive(Clone, PartialEq)]
pub enum Any<AppTx: Message> {
    Connect(Connect),
    Status(Status),
    Block(Block),
    Consensus(ConsensusMessage),
    Request(RequestMessage),
    Transaction(AnyTx<AppTx>),
}

#[derive(Clone, PartialEq, Debug)]
pub enum AnyTx<AppTx: Message> {
    Service(ServiceTx),
    Application(AppTx),
}

#[derive(Clone, PartialEq, Debug)]
pub enum ServiceTx {
    ConfigChange(ConfigMessage),
}

#[derive(Clone, PartialEq)]
pub enum ConfigMessage {
    ConfigPropose(ConfigPropose),
    ConfigVote(ConfigVote),
}

#[derive(Clone, PartialEq)]
pub enum ConsensusMessage {
    Propose(Propose),
    Prevote(Prevote),
    Precommit(Precommit),
}

#[derive(Clone, PartialEq)]
pub enum RequestMessage {
    Propose(RequestPropose),
    Transactions(RequestTransactions),
    Prevotes(RequestPrevotes),
    Precommits(RequestPrecommits),
    Peers(RequestPeers),
    Block(RequestBlock),
}

impl RequestMessage {
    pub fn from(&self) -> &PublicKey {
        match *self {
            RequestMessage::Propose(ref msg) => msg.from(),
            RequestMessage::Transactions(ref msg) => msg.from(),
            RequestMessage::Prevotes(ref msg) => msg.from(),
            RequestMessage::Precommits(ref msg) => msg.from(),
            RequestMessage::Peers(ref msg) => msg.from(),
            RequestMessage::Block(ref msg) => msg.from(),
        }
    }

    pub fn to(&self) -> &PublicKey {
        match *self {
            RequestMessage::Propose(ref msg) => msg.to(),
            RequestMessage::Transactions(ref msg) => msg.to(),
            RequestMessage::Prevotes(ref msg) => msg.to(),
            RequestMessage::Precommits(ref msg) => msg.to(),
            RequestMessage::Peers(ref msg) => msg.to(),
            RequestMessage::Block(ref msg) => msg.to(),
        }
    }

    pub fn time(&self) -> Timespec {
        match *self {
            RequestMessage::Propose(ref msg) => msg.time(),
            RequestMessage::Transactions(ref msg) => msg.time(),
            RequestMessage::Prevotes(ref msg) => msg.time(),
            RequestMessage::Precommits(ref msg) => msg.time(),
            RequestMessage::Peers(ref msg) => msg.time(),
            RequestMessage::Block(ref msg) => msg.time(),
        }
    }

    pub fn verify(&self, public_key: &PublicKey) -> bool {
        match *self {
            RequestMessage::Propose(ref msg) => msg.verify(public_key),
            RequestMessage::Transactions(ref msg) => msg.verify(public_key),
            RequestMessage::Prevotes(ref msg) => msg.verify(public_key),
            RequestMessage::Precommits(ref msg) => msg.verify(public_key),
            RequestMessage::Peers(ref msg) => msg.verify(public_key),
            RequestMessage::Block(ref msg) => msg.verify(public_key),
        }
    }

    pub fn raw(&self) -> &RawMessage {
        match *self {
            RequestMessage::Propose(ref msg) => msg.raw(),
            RequestMessage::Transactions(ref msg) => msg.raw(),
            RequestMessage::Prevotes(ref msg) => msg.raw(),
            RequestMessage::Precommits(ref msg) => msg.raw(),
            RequestMessage::Peers(ref msg) => msg.raw(),
            RequestMessage::Block(ref msg) => msg.raw(),
        }
    }
}

impl fmt::Debug for RequestMessage {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match *self {
            RequestMessage::Propose(ref msg) => write!(fmt, "{:?}", msg),
            RequestMessage::Transactions(ref msg) => write!(fmt, "{:?}", msg),
            RequestMessage::Prevotes(ref msg) => write!(fmt, "{:?}", msg),
            RequestMessage::Precommits(ref msg) => write!(fmt, "{:?}", msg),
            RequestMessage::Peers(ref msg) => write!(fmt, "{:?}", msg),
            RequestMessage::Block(ref msg) => write!(fmt, "{:?}", msg),
        }
    }
}

impl ConfigMessage {
    pub fn from(&self) -> &PublicKey {
        match *self {
            ConfigMessage::ConfigPropose(ref msg) => msg.from(),
            ConfigMessage::ConfigVote(ref msg) => msg.from(),
        }
    }

    pub fn height(&self) -> u64 {
        match *self {
            ConfigMessage::ConfigPropose(ref msg) => msg.height(),
            ConfigMessage::ConfigVote(ref msg) => msg.height(),
        }
    }

    pub fn raw(&self) -> &RawMessage {
        match *self {
            ConfigMessage::ConfigPropose(ref msg) => msg.raw(),
            ConfigMessage::ConfigVote(ref msg) => msg.raw(),
        }
    }

    pub fn verify(&self, public_key: &PublicKey) -> bool {
        match *self {
            ConfigMessage::ConfigPropose(ref msg) => msg.verify(public_key),
            ConfigMessage::ConfigVote(ref msg) => msg.verify(public_key),
        }
    }

    pub fn hash(&self) -> Hash {
        match *self {
            ConfigMessage::ConfigPropose(ref msg) => msg.hash(),
            ConfigMessage::ConfigVote(ref msg) => msg.hash(),
        }
    }
}

impl fmt::Debug for ConfigMessage {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match *self {
            ConfigMessage::ConfigPropose(ref msg) => write!(fmt, "{:?}", msg),
            ConfigMessage::ConfigVote(ref msg) => write!(fmt, "{:?}", msg),
        }
    }
}

impl ConsensusMessage {
    pub fn validator(&self) -> u32 {
        match *self {
            ConsensusMessage::Propose(ref msg) => msg.validator(),
            ConsensusMessage::Prevote(ref msg) => msg.validator(),
            ConsensusMessage::Precommit(ref msg) => msg.validator(),
        }
    }

    pub fn height(&self) -> u64 {
        match *self {
            ConsensusMessage::Propose(ref msg) => msg.height(),
            ConsensusMessage::Prevote(ref msg) => msg.height(),
            ConsensusMessage::Precommit(ref msg) => msg.height(),
        }
    }

    pub fn round(&self) -> u32 {
        match *self {
            ConsensusMessage::Propose(ref msg) => msg.round(),
            ConsensusMessage::Prevote(ref msg) => msg.round(),
            ConsensusMessage::Precommit(ref msg) => msg.round(),
        }
    }

    pub fn raw(&self) -> &RawMessage {
        match *self {
            ConsensusMessage::Propose(ref msg) => msg.raw(),
            ConsensusMessage::Prevote(ref msg) => msg.raw(),
            ConsensusMessage::Precommit(ref msg) => msg.raw(),
        }
    }

    pub fn verify(&self, public_key: &PublicKey) -> bool {
        match *self {
            ConsensusMessage::Propose(ref msg) => msg.verify(public_key),
            ConsensusMessage::Prevote(ref msg) => msg.verify(public_key),
            ConsensusMessage::Precommit(ref msg) => msg.verify(public_key),
        }
    }
}

impl fmt::Debug for ConsensusMessage {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match *self {
            ConsensusMessage::Propose(ref msg) => write!(fmt, "{:?}", msg),
            ConsensusMessage::Prevote(ref msg) => write!(fmt, "{:?}", msg),
            ConsensusMessage::Precommit(ref msg) => write!(fmt, "{:?}", msg),
        }
    }
}

impl<Tx: Message> Any<Tx> {
    pub fn from_raw(raw: RawMessage) -> Result<Any<Tx>, Error> {
        // TODO: check input message size
        Ok(match raw.message_type() {
            CONNECT_MESSAGE_ID => Any::Connect(Connect::from_raw(raw)?),
            STATUS_MESSAGE_ID => Any::Status(Status::from_raw(raw)?),
            BLOCK_MESSAGE_ID => Any::Block(Block::from_raw(raw)?),

            PROPOSE_MESSAGE_ID => {
                Any::Consensus(ConsensusMessage::Propose(Propose::from_raw(raw)?))
            }
            PREVOTE_MESSAGE_ID => {
                Any::Consensus(ConsensusMessage::Prevote(Prevote::from_raw(raw)?))
            }
            PRECOMMIT_MESSAGE_ID => {
                Any::Consensus(ConsensusMessage::Precommit(Precommit::from_raw(raw)?))
            }

            REQUEST_PROPOSE_MESSAGE_ID => {
                Any::Request(RequestMessage::Propose(RequestPropose::from_raw(raw)?))
            }
            REQUEST_TRANSACTIONS_MESSAGE_ID => {
                Any::Request(RequestMessage::Transactions(RequestTransactions::from_raw(raw)?))
            }
            REQUEST_PREVOTES_MESSAGE_ID => {
                Any::Request(RequestMessage::Prevotes(RequestPrevotes::from_raw(raw)?))
            }
            REQUEST_PRECOMMITS_MESSAGE_ID => {
                Any::Request(RequestMessage::Precommits(RequestPrecommits::from_raw(raw)?))
            }
            REQUEST_PEERS_MESSAGE_ID => {
                Any::Request(RequestMessage::Peers(RequestPeers::from_raw(raw)?))
            }
            REQUEST_BLOCK_MESSAGE_ID => {
                Any::Request(RequestMessage::Block(RequestBlock::from_raw(raw)?))
            }
            _ => Any::Transaction(AnyTx::from_raw(raw)?),
        })
    }
}

impl ServiceTx {
    pub fn from(&self) -> &PublicKey {
        match *self {
            ServiceTx::ConfigChange(ref msg) => msg.from(),
        }
    }

    pub fn verify(&self) -> bool {
        match *self {
            ServiceTx::ConfigChange(ref msg) => msg.verify(msg.from()),
        }
    }

    pub fn raw(&self) -> &RawMessage {
        match *self {
            ServiceTx::ConfigChange(ref msg) => msg.raw(),
        }
    }
}

impl<AppTx: Message> AnyTx<AppTx> {
    pub fn verify<B>(&self) -> bool
        where B: Blockchain<Transaction = AppTx>
    {
        match *self {
            AnyTx::Application(ref msg) => B::verify_tx(msg),
            AnyTx::Service(ref msg) => msg.verify(),
        }
    }

    pub fn raw(&self) -> &RawMessage {
        match *self {
            AnyTx::Application(ref msg) => msg.raw(),
            AnyTx::Service(ref msg) => msg.raw(),
        }
    }

    pub fn from_raw(raw: RawMessage) -> Result<AnyTx<AppTx>, Error> {
        // TODO: check input message size
        Ok(match raw.message_type() {
            CONFIG_PROPOSE_MESSAGE_ID => {
                    AnyTx::Service(
                        ServiceTx::ConfigChange(
                            ConfigMessage::ConfigPropose(ConfigPropose::from_raw(raw)?)
                        )
                    )
            }
            CONFIG_VOTE_MESSAGE_ID => {
                    AnyTx::Service(
                        ServiceTx::ConfigChange(
                            ConfigMessage::ConfigVote(ConfigVote::from_raw(raw)?)
                        )
                    )
            }
            _ => AnyTx::Application(AppTx::from_raw(raw)?)
        })
    }

    pub fn hash(&self) -> Hash {
        self.raw().hash()
    }
}

impl<Tx: Message> fmt::Debug for Any<Tx> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match *self {
            Any::Connect(ref msg) => write!(fmt, "{:?}", msg),
            Any::Status(ref msg) => write!(fmt, "{:?}", msg),
            Any::Consensus(ref msg) => write!(fmt, "{:?}", msg),
            Any::Request(ref msg) => write!(fmt, "{:?}", msg),
            Any::Transaction(ref msg) => write!(fmt, "{:?}", msg),
            Any::Block(ref msg) => write!(fmt, "{:?}", msg),
        }
    }
}
