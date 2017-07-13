use bit_vec;

use std::fmt;

use crypto::PublicKey;
use encoding::Error;

pub use self::raw::{RawMessage, MessageWriter, MessageBuffer, Message, FromRaw,
                    HEADER_LENGTH, PROTOCOL_MAJOR_VERSION, TEST_NETWORK_ID};
pub use self::protocol::*;

#[macro_use]
mod spec;
mod raw;
mod protocol;

#[cfg(test)]
mod tests;

pub type BitVec = bit_vec::BitVec;

// TODO: implement common methods for enum types (hash, raw, from_raw, verify)
// TODO: use macro for implementing enums

pub type RawTransaction = RawMessage;

#[derive(Debug, Clone, PartialEq)]
pub enum Any {
    Connect(Connect),
    Status(Status),
    Block(Block),
    Consensus(ConsensusMessage),
    Request(RequestMessage),
    Transaction(RawTransaction),
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
    Peers(RequestPeers),
    Block(RequestBlock),
}

impl RequestMessage {
    pub fn from(&self) -> &PublicKey {
        match *self {
            RequestMessage::Propose(ref msg) => msg.from(),
            RequestMessage::Transactions(ref msg) => msg.from(),
            RequestMessage::Prevotes(ref msg) => msg.from(),
            RequestMessage::Peers(ref msg) => msg.from(),
            RequestMessage::Block(ref msg) => msg.from(),
        }
    }

    pub fn to(&self) -> &PublicKey {
        match *self {
            RequestMessage::Propose(ref msg) => msg.to(),
            RequestMessage::Transactions(ref msg) => msg.to(),
            RequestMessage::Prevotes(ref msg) => msg.to(),
            RequestMessage::Peers(ref msg) => msg.to(),
            RequestMessage::Block(ref msg) => msg.to(),
        }
    }

    #[cfg_attr(feature="flame_profile", flame)]
    pub fn verify(&self, public_key: &PublicKey) -> bool {
        match *self {
            RequestMessage::Propose(ref msg) => msg.verify_signature(public_key),
            RequestMessage::Transactions(ref msg) => msg.verify_signature(public_key),
            RequestMessage::Prevotes(ref msg) => msg.verify_signature(public_key),
            RequestMessage::Peers(ref msg) => msg.verify_signature(public_key),
            RequestMessage::Block(ref msg) => msg.verify_signature(public_key),
        }
    }

    pub fn raw(&self) -> &RawMessage {
        match *self {
            RequestMessage::Propose(ref msg) => msg.raw(),
            RequestMessage::Transactions(ref msg) => msg.raw(),
            RequestMessage::Prevotes(ref msg) => msg.raw(),
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
            RequestMessage::Peers(ref msg) => write!(fmt, "{:?}", msg),
            RequestMessage::Block(ref msg) => write!(fmt, "{:?}", msg),
        }
    }
}

impl ConsensusMessage {
    pub fn validator(&self) -> u16 {
        match *self {
            ConsensusMessage::Propose(ref msg) => msg.validator(),
            ConsensusMessage::Prevote(ref msg) => msg.validator(),
            ConsensusMessage::Precommit(ref msg) => msg.validator(),
        }
    }

    /// Returns height of the message.
    pub fn height(&self) -> u64 {
        match *self {
            ConsensusMessage::Propose(ref msg) => msg.height(),
            ConsensusMessage::Prevote(ref msg) => msg.height(),
            ConsensusMessage::Precommit(ref msg) => msg.height(),
        }
    }

    /// Returns round of the message.
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
            ConsensusMessage::Propose(ref msg) => msg.verify_signature(public_key),
            ConsensusMessage::Prevote(ref msg) => msg.verify_signature(public_key),
            ConsensusMessage::Precommit(ref msg) => msg.verify_signature(public_key),
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

impl Any {
    pub fn from_raw(raw: RawMessage) -> Result<Any, Error> {
        // TODO: check input message size
        let msg =
            if raw.service_id() == CONSENSUS {
                match raw.message_type() {
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
                    REQUEST_TRANSACTIONS_MESSAGE_ID => Any::Request(RequestMessage::Transactions(RequestTransactions::from_raw(raw)?)),
                    REQUEST_PREVOTES_MESSAGE_ID => {
                        Any::Request(RequestMessage::Prevotes(RequestPrevotes::from_raw(raw)?))
                    }
                    REQUEST_PEERS_MESSAGE_ID => {
                        Any::Request(RequestMessage::Peers(RequestPeers::from_raw(raw)?))
                    }
                    REQUEST_BLOCK_MESSAGE_ID => {
                        Any::Request(RequestMessage::Block(RequestBlock::from_raw(raw)?))
                    }
                    message_type => {
                        return Err(Error::IncorrectMessageType{ message_type });
                    }
                }
            } else {
                Any::Transaction(raw)
            };
        Ok(msg)
    }
}
