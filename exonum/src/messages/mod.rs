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

use super::crypto::PublicKey;

pub use self::raw::{RawMessage, MessageWriter, MessageBuffer, Message, HEADER_SIZE};
pub use self::error::Error;
pub use self::fields::{Field, SegmentField};
pub use self::protocol::*;

// TODO: implement common methods for enum types (hash, raw, from_raw, verify)
// TODO: use macro for implementing enums

#[derive(Clone, PartialEq)]
pub enum Any<Tx: Message> {
    Connect(Connect),
    Status(Status),
    Block(Block),
    Consensus(ConsensusMessage),
    Request(RequestMessage),
    Transaction(Tx),
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

// #[derive(Clone, PartialEq)]
// pub enum TxMessage {
//     Issue(TxIssue),
//     Transfer(TxTransfer),
//     VoteValidator(TxVoteValidator),
//     VoteConfig(TxVoteConfig),
// }

// impl TxMessage {
//     pub fn hash(&self) -> Hash {
//         match *self {
//             TxMessage::Issue(ref msg) => msg.hash(),
//             TxMessage::Transfer(ref msg) => msg.hash(),
//             TxMessage::VoteValidator(ref msg) => msg.hash(),
//             TxMessage::VoteConfig(ref msg) => msg.hash()
//         }
//     }

//     pub fn raw(&self) -> &RawMessage {
//         match *self {
//             TxMessage::Issue(ref msg) => msg.raw(),
//             TxMessage::Transfer(ref msg) => msg.raw(),
//             TxMessage::VoteValidator(ref msg) => msg.raw(),
//             TxMessage::VoteConfig(ref msg) => msg.raw()
//         }
//     }

//     pub fn from_raw(raw: RawMessage) -> Result<TxMessage, Error> {
//         // TODO: check input message size
//         Ok(match raw.message_type() {
//             TxIssue::MESSAGE_TYPE => TxMessage::Issue(TxIssue::from_raw(raw)?),
//             TxTransfer::MESSAGE_TYPE => TxMessage::Transfer(TxTransfer::from_raw(raw)?),
//             TxVoteValidator::MESSAGE_TYPE => TxMessage::VoteValidator(TxVoteValidator::from_raw(raw)?),
//             TxVoteConfig::MESSAGE_TYPE => TxMessage::VoteConfig(TxVoteConfig::from_raw(raw)?),
//             _ => {
//                 // TODO: use result here
//                 panic!("unrecognized message type");
//             }
//         })
//     }
// }

// impl fmt::Debug for TxMessage {
//     fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
//         match *self {
//             TxMessage::Issue(ref msg) => write!(fmt, "{:?}", msg),
//             TxMessage::Transfer(ref msg) => write!(fmt, "{:?}", msg),
//             TxMessage::VoteValidator(ref msg) => write!(fmt, "{:?}", msg),
//             TxMessage::VoteConfig(ref msg) => write!(fmt, "{:?}", msg)
//         }
//     }
// }

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
            BLOCK_MESSAGE_ID => {
                info!("Block len {}", raw.len());
                Any::Block(Block::from_raw(raw)?)
            }

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
            _ => Any::Transaction(Tx::from_raw(raw)?),
        })
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
