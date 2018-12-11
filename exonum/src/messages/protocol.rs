// Copyright 2018 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Messages used in the Exonum consensus algorithm.
//!
//! Every message, unless stated otherwise, is checked by the same set of rules. The message is
//! ignored if it
//!     * is sent from a lower height than the current one
//!     * contains incorrect validator id
//!     * is signed with incorrect signature
//!
//! Specific nuances are described in each message documentation and typically consist of three
//! parts:
//!     * validation - additional checks before processing
//!     * processing - how message is processed and result of the processing
//!     * generation - in which cases message is generated

use bit_vec::BitVec;
use chrono::{DateTime, Utc};
use failure;

use std::{borrow::Cow, fmt::Debug, mem};

use super::{BinaryForm, RawTransaction, ServiceTransaction, Signed, SignedMessage};
use blockchain;
use crypto::{CryptoHash, Hash, PublicKey, SecretKey, PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH};
use helpers::{Height, Round, ValidatorId};
use proto;
use storage::{proof_list_index as merkle, StorageValue};

/// `SignedMessage` size with zero bytes payload.
#[doc(hidden)]
pub const EMPTY_SIGNED_MESSAGE_SIZE: usize =
    PUBLIC_KEY_LENGTH + SIGNATURE_LENGTH + mem::size_of::<u8>() * 2;

/// `Signed<TransactionsResponse>` size without transactions inside.
#[doc(hidden)]
pub const TRANSACTION_RESPONSE_EMPTY_SIZE: usize =
    EMPTY_SIGNED_MESSAGE_SIZE + PUBLIC_KEY_LENGTH + mem::size_of::<u8>() * 4;

/// `Signed<RawTransaction>` size with empty transaction inside.
pub const RAW_TRANSACTION_EMPTY_SIZE: usize = EMPTY_SIGNED_MESSAGE_SIZE + mem::size_of::<u16>() * 2;

/// Connect to a node.
///
/// ### Validation
/// The message is ignored if its time is earlier than in the previous
/// `Connect` message received from the same peer.
///
/// ### Processing
/// Connect to the peer.
///
/// ### Generation
/// A node sends `Connect` message to all known addresses during
/// initialization. Additionally, the node responds by its own `Connect`
/// message after receiving `node::Event::Connected`.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[exonum(pb = "proto::Connect", crate = "crate")]
pub struct Connect {
    /// The node's address.
    pub_addr: String,
    /// Time when the message was created.
    time: DateTime<Utc>,
    /// String containing information about this node including Exonum, Rust and OS versions.
    user_agent: String,
}

impl Connect {
    /// Create new `Connect` message.
    pub fn new(addr: &str, time: DateTime<Utc>, user_agent: &str) -> Self {
        Connect {
            pub_addr: addr.to_owned(),
            time,
            user_agent: user_agent.to_owned(),
        }
    }

    /// The node's address.
    pub fn pub_addr(&self) -> &str {
        &self.pub_addr
    }

    /// Time when the message was created.
    pub fn time(&self) -> DateTime<Utc> {
        self.time
    }

    /// String containing information about this node including Exonum, Rust and OS versions.
    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }
}

/// Current node status.
///
/// ### Validation
/// The message is ignored if its signature is incorrect or its `height` is
/// lower than a node's height.
///
/// ### Processing
/// If the message's `height` number is bigger than a node's one, then
/// `BlockRequest` with current node's height is sent in reply.
///
/// ### Generation
/// `Status` message is broadcast regularly with the timeout controlled by
/// `blockchain::ConsensusConfig::status_timeout`. Also, it is broadcast
/// after accepting a new block.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[exonum(pb = "proto::Status", crate = "crate")]
pub struct Status {
    /// The height to which the message is related.
    height: Height,
    /// Hash of the last committed block.
    last_hash: Hash,
}

impl Status {
    /// Create new `Status` message.
    pub fn new(height: Height, last_hash: &Hash) -> Self {
        Self {
            height,
            last_hash: *last_hash,
        }
    }

    /// The height to which the message is related.
    pub fn height(&self) -> Height {
        self.height
    }

    /// Hash of the last committed block.
    pub fn last_hash(&self) -> &Hash {
        &self.last_hash
    }
}

/// Proposal for a new block.
///
/// ### Validation
/// The message is ignored if it
///     * contains incorrect `prev_hash`
///     * is sent by non-leader
///     * contains already committed transactions
///     * is already known
///
/// ### Processing
/// If the message contains unknown transactions, then `TransactionsRequest`
/// is sent in reply.  Otherwise `Prevote` is broadcast.
///
/// ### Generation
/// A node broadcasts `Propose` if it is a leader and is not locked for a
/// different proposal. Also `Propose` can be sent as response to
/// `ProposeRequest`.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[exonum(pb = "proto::Propose", crate = "crate")]
pub struct Propose {
    /// The validator id.
    validator: ValidatorId,
    /// The height to which the message is related.
    height: Height,
    /// The round to which the message is related.
    round: Round,
    /// Hash of the previous block.
    prev_hash: Hash,
    /// The list of transactions to include in the next block.
    transactions: Vec<Hash>,
}

impl Propose {
    /// Create new `Propose` message.
    pub fn new(
        validator: ValidatorId,
        height: Height,
        round: Round,
        prev_hash: &Hash,
        transactions: &[Hash],
    ) -> Self {
        Self {
            validator,
            height,
            round,
            prev_hash: *prev_hash,
            transactions: transactions.to_vec(),
        }
    }

    /// The validator id.
    pub fn validator(&self) -> ValidatorId {
        self.validator
    }
    /// The height to which the message is related.
    pub fn height(&self) -> Height {
        self.height
    }
    /// The round to which the message is related.
    pub fn round(&self) -> Round {
        self.round
    }
    /// Hash of the previous block.
    pub fn prev_hash(&self) -> &Hash {
        &self.prev_hash
    }
    /// The list of transactions to include in the next block.
    pub fn transactions(&self) -> &[Hash] {
        &self.transactions
    }
}

/// Pre-vote for a new block.
///
/// ### Validation
/// A node panics if it has already sent a different `Prevote` for the same
/// round.
///
/// ### Processing
/// Pre-vote is added to the list of known votes for the same proposal.  If
/// `locked_round` number from the message is bigger than in a node state,
/// then a node replies with `PrevotesRequest`.  If there are unknown
/// transactions in the propose specified by `propose_hash`,
/// `TransactionsRequest` is sent in reply.  Otherwise if all transactions
/// are known and there are +2/3 pre-votes, then a node is locked to that
/// proposal and `Precommit` is broadcast.
///
/// ### Generation
/// A node broadcasts `Prevote` in response to `Propose` when it has
/// received all the transactions.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[exonum(pb = "proto::Prevote", crate = "crate")]
pub struct Prevote {
    /// The validator id.
    validator: ValidatorId,
    /// The height to which the message is related.
    height: Height,
    /// The round to which the message is related.
    round: Round,
    /// Hash of the corresponding `Propose`.
    propose_hash: Hash,
    /// Locked round.
    locked_round: Round,
}

impl Prevote {
    /// Create new `Prevote` message.
    pub fn new(
        validator: ValidatorId,
        height: Height,
        round: Round,
        propose_hash: &Hash,
        locked_round: Round,
    ) -> Self {
        Self {
            validator,
            height,
            round,
            propose_hash: *propose_hash,
            locked_round,
        }
    }

    /// The validator id.
    pub fn validator(&self) -> ValidatorId {
        self.validator
    }
    /// The height to which the message is related.
    pub fn height(&self) -> Height {
        self.height
    }
    /// The round to which the message is related.
    pub fn round(&self) -> Round {
        self.round
    }
    /// Hash of the corresponding `Propose`.
    pub fn propose_hash(&self) -> &Hash {
        &self.propose_hash
    }
    /// Locked round.
    pub fn locked_round(&self) -> Round {
        self.locked_round
    }
}

/// Pre-commit for a proposal.
///
/// ### Validation
/// A node panics if it has already sent a different `Precommit` for the
/// same round.
///
/// ### Processing
/// Pre-commit is added to the list of known pre-commits.  If a proposal is
/// unknown to the node, `ProposeRequest` is sent in reply.  If `round`
/// number from the message is bigger than a node's "locked round", then a
/// node replies with `PrevotesRequest`.  If there are unknown transactions,
/// then `TransactionsRequest` is sent in reply.  If a validator receives
/// +2/3 precommits for the same proposal with the same block_hash, then
/// block is executed and `Status` is broadcast.
///
/// ### Generation
/// A node broadcasts `Precommit` in response to `Prevote` if there are +2/3
/// pre-votes and no unknown transactions.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, Serialize, Deserialize, ProtobufConvert)]
#[exonum(pb = "proto::Precommit", crate = "crate")]
pub struct Precommit {
    /// The validator id.
    validator: ValidatorId,
    /// The height to which the message is related.
    height: Height,
    /// The round to which the message is related.
    round: Round,
    /// Hash of the corresponding `Propose`.
    propose_hash: Hash,
    /// Hash of the new block.
    block_hash: Hash,
    /// Time of the `Precommit`.
    time: DateTime<Utc>,
}

impl Precommit {
    /// Create new `Precommit` message.
    pub fn new(
        validator: ValidatorId,
        height: Height,
        round: Round,
        propose_hash: &Hash,
        block_hash: &Hash,
        time: DateTime<Utc>,
    ) -> Self {
        Self {
            validator,
            height,
            round,
            propose_hash: *propose_hash,
            block_hash: *block_hash,
            time,
        }
    }
    /// The validator id.
    pub fn validator(&self) -> ValidatorId {
        self.validator
    }
    /// The height to which the message is related.
    pub fn height(&self) -> Height {
        self.height
    }
    /// The round to which the message is related.
    pub fn round(&self) -> Round {
        self.round
    }
    /// Hash of the corresponding `Propose`.
    pub fn propose_hash(&self) -> &Hash {
        &self.propose_hash
    }
    /// Hash of the new block.
    pub fn block_hash(&self) -> &Hash {
        &self.block_hash
    }
    /// Time of the `Precommit`.
    pub fn time(&self) -> DateTime<Utc> {
        self.time
    }
}

/// Information about a block.
///
/// ### Validation
/// The message is ignored if
///     * its `to` field corresponds to a different node
///     * the `block`, `transaction` and `precommits` fields cannot be
///     parsed or verified
///
/// ### Processing
/// The block is added to the blockchain.
///
/// ### Generation
/// The message is sent as response to `BlockRequest`.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[exonum(pb = "proto::BlockResponse", crate = "crate")]
pub struct BlockResponse {
    /// Public key of the recipient.
    to: PublicKey,
    /// Block header.
    block: blockchain::Block,
    /// List of pre-commits.
    precommits: Vec<Vec<u8>>,
    /// List of the transaction hashes.
    transactions: Vec<Hash>,
}

impl BlockResponse {
    /// Create new `BlockResponse` message.
    pub fn new(
        to: &PublicKey,
        block: blockchain::Block,
        precommits: Vec<Vec<u8>>,
        transactions: &[Hash],
    ) -> Self {
        Self {
            to: *to,
            block,
            precommits,
            transactions: transactions.to_vec(),
        }
    }

    /// Public key of the recipient.
    pub fn to(&self) -> &PublicKey {
        &self.to
    }
    /// Block header.
    pub fn block(&self) -> blockchain::Block {
        self.block.clone()
    }
    /// List of pre-commits.
    pub fn precommits(&self) -> Vec<Vec<u8>> {
        self.precommits.clone()
    }
    /// List of the transaction hashes.
    pub fn transactions(&self) -> &[Hash] {
        &self.transactions
    }
}

/// Information about the transactions.
///
/// ### Validation
/// The message is ignored if
///     * its `to` field corresponds to a different node
///     * the `transactions` field cannot be parsed or verified
///
/// ### Processing
/// Returns information about the transactions requested by the hash.
///
/// ### Generation
/// The message is sent as response to `TransactionsRequest`.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[exonum(pb = "proto::TransactionsResponse", crate = "crate")]
pub struct TransactionsResponse {
    /// Public key of the recipient.
    to: PublicKey,
    /// List of the transactions.
    transactions: Vec<Vec<u8>>,
}

impl TransactionsResponse {
    /// Create new `TransactionsResponse` message.
    pub fn new(to: &PublicKey, transactions: Vec<Vec<u8>>) -> Self {
        Self {
            to: *to,
            transactions,
        }
    }

    /// Public key of the recipient.
    pub fn to(&self) -> &PublicKey {
        &self.to
    }
    /// List of the transactions.
    pub fn transactions(&self) -> Vec<Vec<u8>> {
        self.transactions.clone()
    }
}

/// Request for the `Propose`.
///
/// ### Validation
/// The message is ignored if its `height` is not equal to the node's
/// height.
///
/// ### Processing
/// `Propose` is sent as the response.
///
/// ### Generation
/// A node can send `ProposeRequest` during `Precommit` and `Prevote`
/// handling.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[exonum(pb = "proto::ProposeRequest", crate = "crate")]
pub struct ProposeRequest {
    /// Public key of the recipient.
    to: PublicKey,
    /// The height to which the message is related.
    height: Height,
    /// Hash of the `Propose`.
    propose_hash: Hash,
}

impl ProposeRequest {
    /// Create new `ProposeRequest`.
    pub fn new(to: &PublicKey, height: Height, propose_hash: &Hash) -> Self {
        Self {
            to: *to,
            height,
            propose_hash: *propose_hash,
        }
    }

    /// Public key of the recipient.
    pub fn to(&self) -> &PublicKey {
        &self.to
    }
    /// The height to which the message is related.
    pub fn height(&self) -> Height {
        self.height
    }
    /// Hash of the `Propose`.
    pub fn propose_hash(&self) -> &Hash {
        &self.propose_hash
    }
}

/// Request for transactions by hash.
///
/// ### Processing
/// Requested transactions are sent to the recipient.
///
/// ### Generation
/// This message can be sent during `Propose`, `Prevote` and `Precommit`
/// handling.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[exonum(pb = "proto::TransactionsRequest", crate = "crate")]
pub struct TransactionsRequest {
    /// Public key of the recipient.
    to: PublicKey,
    /// The list of the transaction hashes.
    txs: Vec<Hash>,
}

impl TransactionsRequest {
    /// Create new `TransactionsRequest`.
    pub fn new(to: &PublicKey, txs: &[Hash]) -> Self {
        Self {
            to: *to,
            txs: txs.to_vec(),
        }
    }

    /// Public key of the recipient.
    pub fn to(&self) -> &PublicKey {
        &self.to
    }
    /// The list of the transaction hashes.
    pub fn txs(&self) -> &[Hash] {
        &self.txs
    }
}

/// Request for pre-votes.
///
/// ### Validation
/// The message is ignored if its `height` is not equal to the node's
/// height.
///
/// ### Processing
/// The requested pre-votes are sent to the recipient.
///
/// ### Generation
/// This message can be sent during `Prevote` and `Precommit` handling.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[exonum(pb = "proto::PrevotesRequest", crate = "crate")]
pub struct PrevotesRequest {
    /// Public key of the recipient.
    to: PublicKey,
    /// The height to which the message is related.
    height: Height,
    /// The round to which the message is related.
    round: Round,
    /// Hash of the `Propose`.
    propose_hash: Hash,
    /// The list of validators that send pre-votes.
    validators: BitVec,
}

impl PrevotesRequest {
    /// Create new `PrevotesRequest`.
    pub fn new(
        to: &PublicKey,
        height: Height,
        round: Round,
        propose_hash: &Hash,
        validators: BitVec,
    ) -> Self {
        Self {
            to: *to,
            height,
            round,
            propose_hash: *propose_hash,
            validators,
        }
    }

    /// Public key of the recipient.
    pub fn to(&self) -> &PublicKey {
        &self.to
    }
    /// The height to which the message is related.
    pub fn height(&self) -> Height {
        self.height
    }
    /// The round to which the message is related.
    pub fn round(&self) -> Round {
        self.round
    }
    /// Hash of the `Propose`.
    pub fn propose_hash(&self) -> &Hash {
        &self.propose_hash
    }
    /// The list of validators that send pre-votes.
    pub fn validators(&self) -> BitVec {
        self.validators.clone()
    }
}

/// Request connected peers from a node.
///
/// ### Validation
/// Request is considered valid if the sender of the message on the network
/// level corresponds to the `from` field.
///
/// ### Processing
/// Peer `Connect` messages are sent to the recipient.
///
/// ### Generation
/// `PeersRequest` message is sent regularly with the timeout controlled by
/// `blockchain::ConsensusConfig::peers_timeout`.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[exonum(pb = "proto::PeersRequest", crate = "crate")]
pub struct PeersRequest {
    /// Public key of the recipient.
    to: PublicKey,
}

impl PeersRequest {
    /// Create new `PeersRequest`.
    pub fn new(to: &PublicKey) -> Self {
        Self { to: *to }
    }
    /// Public key of the recipient.
    pub fn to(&self) -> &PublicKey {
        &self.to
    }
}

/// Request for the block with the given `height`.
///
/// ### Validation
/// The message is ignored if its `height` is bigger than the node's one.
///
/// ### Processing
/// `BlockResponse` message is sent as the response.
///
/// ### Generation
/// This message can be sent during `Status` processing.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[exonum(pb = "proto::BlockRequest", crate = "crate")]
pub struct BlockRequest {
    /// Public key of the recipient.
    to: PublicKey,
    /// The height to which the message is related.
    height: Height,
}

impl BlockRequest {
    /// Create new `BlockRequest`.
    pub fn new(to: &PublicKey, height: Height) -> Self {
        Self { to: *to, height }
    }
    /// Public key of the recipient.
    pub fn to(&self) -> &PublicKey {
        &self.to
    }
    /// The height to which the message is related.
    pub fn height(&self) -> Height {
        self.height
    }
}

impl BlockResponse {
    /// Verify Merkle root of transactions in the block.
    pub fn verify_tx_hash(&self) -> bool {
        *self.block().tx_hash() == merkle::root_hash(self.transactions())
    }
}

impl Precommit {
    /// Verify precommits signature and return it's safer wrapper
    pub(crate) fn verify_precommit(buffer: Vec<u8>) -> Result<Signed<Precommit>, ::failure::Error> {
        let signed = SignedMessage::from_raw_buffer(buffer)?;
        let protocol = Message::deserialize(signed)?;
        ProtocolMessage::try_from(protocol)
            .map_err(|_| format_err!("Couldn't verify precommit from message"))
    }
}

/// Full message constraints list.
#[doc(hidden)]
pub trait ProtocolMessage: Debug + Clone + BinaryForm {
    fn message_type() -> (u8, u8);
    /// Trying to convert `Message` to concrete message,
    /// if ok returns message `Signed<Self>` if fails, returns `Message` back.
    fn try_from(p: Message) -> Result<Signed<Self>, Message>;

    fn into_protocol(this: Signed<Self>) -> Message;

    fn into_message_from_parts(self, sm: SignedMessage) -> Signed<Self>;
}

/// Implement Exonum message protocol.
///
/// Protocol should be described according to format:
/// ```
/// /// type of SignedMessage => new name of Message enum.
/// SignedMessage => Message {
///       // class ID => class name
///       0 => Service {
///            // message = message type ID
///            RawTransaction = 0,
///            Connect = 1,
///            Status = 2,
///            // ...
///        },
///        1 => Consensus {
///            Precommit = 0,
///            Propose = 1,
///            Prevote = 2,
///        },
/// }
/// ```
///
/// Each message should implement `Clone` and `Debug`.
///
macro_rules! impl_protocol {

    (
    $(#[$attr:meta])+
    $signed_message:ident => $protocol_name:ident{
        $($(#[$attr_class:meta])+
        $class_num:expr => $class:ident{
            $(
                $(#[$attr_type:meta])+
                $type:ident = $type_num:expr
            ),+ $(,)*
        } $(,)*)+
    }
    ) => {

        $(
            #[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone)]
            $(#[$attr_class])+
            pub enum $class {
            $(
                $(#[$attr_type])+
                $type(Signed<$type>)
            ),+
            }

            $(

            impl ProtocolMessage for $type {
                fn message_type() -> (u8, u8) {
                    ($class_num, $type_num)
                }

                fn try_from(p: $protocol_name) -> Result<Signed<Self>, $protocol_name> {
                    match p {
                        $protocol_name::$class($class::$type(s)) => Ok(s),
                        p => Err(p)
                    }
                }

                fn into_protocol(this: Signed<Self>) -> $protocol_name {
                    $protocol_name::$class($class::$type(this))
                }

                fn into_message_from_parts(self, sm: SignedMessage) -> Signed<Self> {
                    Signed::new(self, sm)
                }
            }
            )+
        )+

        #[derive(PartialEq, Eq, Debug, Clone)]
        $(#[$attr])+
        pub enum $protocol_name {
            $(
             $(#[$attr_class])+
                $class($class)
            ),+
        }

        impl $protocol_name {
            /// Converts raw `SignedMessage` into concrete `Message` message.
            /// Returns error if fails.
            pub fn deserialize(message: SignedMessage) -> Result<Self, failure::Error> {
                match message.message_class() {
                    $($class_num =>
                        match message.message_type() {
                            $($type_num =>{
                                let payload = $type::decode(message.payload())?;
                                let message = Signed::new(payload, message);
                                Ok($protocol_name::$class($class::$type(message)))
                            }),+
                            _ => bail!("Not found message with this type {}", message.message_type())
                        }
                    ),+
                    _ => bail!("Not found message with this class {}", message.message_class())
                }
            }

            /// Returns reference to inner `SignedMessage`.
            pub fn signed_message(&self) -> &SignedMessage {
                match *self {
                    $(
                        $protocol_name::$class(ref c) => {
                            match *c {
                                $(
                                    $class::$type(ref t) => {
                                        t.signed_message()
                                    }
                                ),+
                            }
                        }
                    ),+
                }
            }
        }
    };
}

impl_protocol! {
    /// Composition of every exonum protocol messages.
    /// This messages used in network p2p communications.
    SignedMessage => Message {
        /// Exonum basic node messages.
        0 => Service {
            /// `RawTransaction` representation.
            RawTransaction = 0,
            /// Handshake to other node.
            Connect = 1,
            /// `Status` information of other node.
            Status = 2,
        },
        /// Exonum consensus specific node messages.
        1 => Consensus {
            /// Consensus `Precommit` message.
            Precommit = 0,
            /// Consensus `Propose` message.
            Propose = 1,
            /// Consensus `Prevote` message.
            Prevote = 2,
        },
        /// Exonum node responses.
        2 => Responses {
            /// Information about transactions, that sent as response to `TransactionsRequest`.
            TransactionsResponse = 0,
            /// Information about block, that sent as response to `BlockRequest`.
            BlockResponse = 1,
        },
        /// Exonum node requests.
        3 => Requests {
            /// Request of some propose which hash is known.
            ProposeRequest = 0,
            /// Request of unknown transactions.
            TransactionsRequest = 1,
            /// Request of prevotes for some propose.
            PrevotesRequest = 2,
            /// Request of peer exchange.
            PeersRequest = 3,
            /// Request of some future block.
            BlockRequest = 4,
        },

    }
}

impl Message {
    /// Creates new protocol message.
    ///
    /// # Panics
    ///
    /// This method can panic on serialization failure.
    pub fn new<T: ProtocolMessage>(
        message: T,
        author: PublicKey,
        secret_key: &SecretKey,
    ) -> Message {
        T::into_protocol(Message::concrete(message, author, secret_key))
    }

    /// Creates new protocol message.
    /// Return concrete `Signed<T>`
    ///
    /// # Panics
    ///
    /// This method can panic on serialization failure.
    pub fn concrete<T: ProtocolMessage>(
        message: T,
        author: PublicKey,
        secret_key: &SecretKey,
    ) -> Signed<T> {
        let value = message.encode().expect("Couldn't serialize data.");
        let (cls, typ) = T::message_type();
        let signed = SignedMessage::new(cls, typ, &value, author, secret_key);
        T::into_message_from_parts(message, signed)
    }

    /// Checks buffer and return instance of `Message`.
    pub fn from_raw_buffer(buffer: Vec<u8>) -> Result<Message, failure::Error> {
        let signed = SignedMessage::from_raw_buffer(buffer)?;
        Self::deserialize(signed)
    }

    /// Creates a new raw transaction message.
    ///
    /// # Panics
    ///
    /// This method can panic on serialization failure.
    pub fn sign_transaction<T>(
        transaction: T,
        service_id: u16,
        public_key: PublicKey,
        secret_key: &SecretKey,
    ) -> Signed<RawTransaction>
    where
        T: Into<ServiceTransaction>,
    {
        let set: ServiceTransaction = transaction.into();
        let raw_tx = RawTransaction::new(service_id, set);
        Self::concrete(raw_tx, public_key, secret_key)
    }
}

impl Requests {
    /// Returns public key of the message recipient.
    pub fn to(&self) -> PublicKey {
        *match *self {
            Requests::ProposeRequest(ref msg) => msg.to(),
            Requests::TransactionsRequest(ref msg) => msg.to(),
            Requests::PrevotesRequest(ref msg) => msg.to(),
            Requests::PeersRequest(ref msg) => msg.to(),
            Requests::BlockRequest(ref msg) => msg.to(),
        }
    }

    /// Returns author public key of the message sender.
    pub fn author(&self) -> PublicKey {
        match *self {
            Requests::ProposeRequest(ref msg) => msg.author(),
            Requests::TransactionsRequest(ref msg) => msg.author(),
            Requests::PrevotesRequest(ref msg) => msg.author(),
            Requests::PeersRequest(ref msg) => msg.author(),
            Requests::BlockRequest(ref msg) => msg.author(),
        }
    }
}

impl Consensus {
    /// Returns author public key of the message sender.
    pub fn author(&self) -> PublicKey {
        match *self {
            Consensus::Propose(ref msg) => msg.author(),
            Consensus::Prevote(ref msg) => msg.author(),
            Consensus::Precommit(ref msg) => msg.author(),
        }
    }

    /// Returns validator id of the message sender.
    pub fn validator(&self) -> ValidatorId {
        match *self {
            Consensus::Propose(ref msg) => msg.validator(),
            Consensus::Prevote(ref msg) => msg.validator(),
            Consensus::Precommit(ref msg) => msg.validator(),
        }
    }

    /// Returns height of the message.
    pub fn height(&self) -> Height {
        match *self {
            Consensus::Propose(ref msg) => msg.height(),
            Consensus::Prevote(ref msg) => msg.height(),
            Consensus::Precommit(ref msg) => msg.height(),
        }
    }

    /// Returns round of the message.
    pub fn round(&self) -> Round {
        match *self {
            Consensus::Propose(ref msg) => msg.round(),
            Consensus::Prevote(ref msg) => msg.round(),
            Consensus::Precommit(ref msg) => msg.round(),
        }
    }
}

impl<T: ProtocolMessage> From<Signed<T>> for Message {
    fn from(other: Signed<T>) -> Self {
        ProtocolMessage::into_protocol(other)
    }
}

impl StorageValue for Message {
    fn into_bytes(self) -> Vec<u8> {
        self.signed_message().raw().to_vec()
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        let message = SignedMessage::from_vec_unchecked(value.into_owned());
        // TODO: Remove additional deserialization. [ECR-2315]
        Message::deserialize(message).unwrap()
    }
}

impl CryptoHash for Message {
    fn hash(&self) -> Hash {
        self.signed_message().hash()
    }
}
