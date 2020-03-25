// Copyright 2020 The Exonum Team
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

pub use self::types::*;

use exonum::{
    crypto::{Hash, PublicKey, PUBLIC_KEY_LENGTH},
    helpers::{Height, Round, ValidatorId},
    merkledb::{BinaryValue, ObjectHash},
    messages::{AnyTx, Precommit, SignedMessage, Verified, SIGNED_MESSAGE_MIN_SIZE},
};

use std::borrow::Cow;

mod types;

/// Size of an empty `TransactionsResponse`.
pub const TX_RES_EMPTY_SIZE: usize = SIGNED_MESSAGE_MIN_SIZE + PUBLIC_KEY_LENGTH + 8;

/// When we add transaction to `TransactionResponse` message we will add some overhead
/// to the message size due to Protobuf. This is the higher bound on this overhead.
///
/// ```text
/// Tx response message size <= TX_RES_EMPTY_SIZE
///    + (tx1 size + TX_RES_PB_OVERHEAD_PAYLOAD)
///    + (tx2 size + TX_RES_PB_OVERHEAD_PAYLOAD)
///    + ...
/// ```
pub const TX_RES_PB_OVERHEAD_PAYLOAD: usize = 8;

/// Service messages.
#[derive(Debug, Clone, PartialEq)]
pub enum Service {
    /// Transaction message.
    AnyTx(Verified<AnyTx>),
    /// Connect message.
    Connect(Verified<Connect>),
    /// Status message.
    Status(Verified<Status>),
}

impl Service {
    fn as_raw(&self) -> &SignedMessage {
        match self {
            Self::AnyTx(ref msg) => msg.as_raw(),
            Self::Connect(ref msg) => msg.as_raw(),
            Self::Status(ref msg) => msg.as_raw(),
        }
    }
}

/// Consensus messages.
#[derive(Debug, Clone, PartialEq)]
pub enum Consensus {
    /// `Precommit` message.
    Precommit(Verified<Precommit>),
    /// `Propose` message.
    Propose(Verified<Propose>),
    /// `Prevote` message.
    Prevote(Verified<Prevote>),
}

impl Consensus {
    fn as_raw(&self) -> &SignedMessage {
        match self {
            Self::Precommit(msg) => msg.as_raw(),
            Self::Propose(msg) => msg.as_raw(),
            Self::Prevote(msg) => msg.as_raw(),
        }
    }
}

/// Response messages.
#[derive(Debug, Clone, PartialEq)]
pub enum Responses {
    /// Transactions response message.
    TransactionsResponse(Verified<TransactionsResponse>),
    /// Block response message.
    BlockResponse(Verified<BlockResponse>),
}

impl Responses {
    fn as_raw(&self) -> &SignedMessage {
        match self {
            Self::TransactionsResponse(msg) => msg.as_raw(),
            Self::BlockResponse(msg) => msg.as_raw(),
        }
    }
}

impl From<Verified<TransactionsResponse>> for Responses {
    fn from(msg: Verified<TransactionsResponse>) -> Self {
        Self::TransactionsResponse(msg)
    }
}

impl From<Verified<BlockResponse>> for Responses {
    fn from(msg: Verified<BlockResponse>) -> Self {
        Self::BlockResponse(msg)
    }
}

/// Request messages.
#[derive(Debug, Clone, PartialEq)]
pub enum Requests {
    /// Propose request message.
    ProposeRequest(Verified<ProposeRequest>),
    /// Transactions request message.
    TransactionsRequest(Verified<TransactionsRequest>),
    /// Prevotes request message.
    PrevotesRequest(Verified<PrevotesRequest>),
    /// Peers request message.
    PeersRequest(Verified<PeersRequest>),
    /// Block request message.
    BlockRequest(Verified<BlockRequest>),
    /// Request of uncommitted transactions.
    PoolTransactionsRequest(Verified<PoolTransactionsRequest>),
}

impl Requests {
    fn as_raw(&self) -> &SignedMessage {
        match self {
            Self::ProposeRequest(msg) => msg.as_raw(),
            Self::TransactionsRequest(msg) => msg.as_raw(),
            Self::PrevotesRequest(msg) => msg.as_raw(),
            Self::PeersRequest(msg) => msg.as_raw(),
            Self::BlockRequest(msg) => msg.as_raw(),
            Self::PoolTransactionsRequest(msg) => msg.as_raw(),
        }
    }
}

/// Representation of the Exonum message which is divided into categories.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    /// Service messages.
    Service(Service),
    /// Consensus messages.
    Consensus(Consensus),
    /// Responses messages.
    Responses(Responses),
    /// Requests messages.
    Requests(Requests),
}

impl Message {
    /// Deserialize message from signed message.
    pub fn from_signed(signed: SignedMessage) -> anyhow::Result<Self> {
        signed.into_verified::<ExonumMessage>().map(From::from)
    }

    /// Checks buffer and returns instance of `Message`.
    pub fn from_raw_buffer(buffer: Vec<u8>) -> anyhow::Result<Self> {
        SignedMessage::from_bytes(buffer.into()).and_then(Self::from_signed)
    }

    /// Get inner `SignedMessage`.
    pub fn as_raw(&self) -> &SignedMessage {
        match self {
            Self::Service(ref msg) => msg.as_raw(),
            Self::Consensus(ref msg) => msg.as_raw(),
            Self::Requests(ref msg) => msg.as_raw(),
            Self::Responses(ref msg) => msg.as_raw(),
        }
    }
}

impl PartialEq<SignedMessage> for Message {
    fn eq(&self, other: &SignedMessage) -> bool {
        self.as_raw() == other
    }
}

macro_rules! impl_message_from_verified {
    ( $($concrete:ident: $category:ident),* ) => {
        $(
            impl From<Verified<$concrete>> for Message {
                fn from(msg: Verified<$concrete>) -> Self {
                    Self::$category($category::$concrete(msg))
                }
            }

            impl std::convert::TryFrom<Message> for Verified<$concrete> {
                type Error = anyhow::Error;

                fn try_from(msg: Message) -> Result<Self, Self::Error> {
                    if let Message::$category($category::$concrete(msg)) = msg {
                        Ok(msg)
                    } else {
                        Err(anyhow::format_err!(
                            "Given message is not a {}::{}",
                            stringify!($category),
                            stringify!($concrete)
                        ))
                    }
                }
            }
        )*

        impl From<Verified<ExonumMessage>> for Message {
            fn from(msg: Verified<ExonumMessage>) -> Self {
                match msg.payload() {
                    $(
                        ExonumMessage::$concrete(_) => {
                            let inner = msg.downcast_map(|payload| match payload {
                                ExonumMessage::$concrete(payload) => payload,
                                _ => unreachable!(),
                            });
                            Self::from(inner)
                        }
                    )*
                }
            }
        }
    };
}

impl_message_from_verified! {
    AnyTx: Service,
    Connect: Service,
    Status: Service,
    Precommit: Consensus,
    Prevote: Consensus,
    Propose: Consensus,
    BlockResponse: Responses,
    TransactionsResponse: Responses,
    BlockRequest: Requests,
    PeersRequest: Requests,
    PrevotesRequest: Requests,
    ProposeRequest: Requests,
    TransactionsRequest: Requests,
    PoolTransactionsRequest: Requests
}

impl Requests {
    /// Returns public key of the message recipient.
    pub fn to(&self) -> PublicKey {
        match self {
            Self::ProposeRequest(msg) => msg.payload().to,
            Self::TransactionsRequest(msg) => msg.payload().to,
            Self::PrevotesRequest(msg) => msg.payload().to,
            Self::PeersRequest(msg) => msg.payload().to,
            Self::BlockRequest(msg) => msg.payload().to,
            Self::PoolTransactionsRequest(msg) => msg.payload().to,
        }
    }

    /// Returns author public key of the message sender.
    pub fn author(&self) -> PublicKey {
        match self {
            Self::ProposeRequest(msg) => msg.author(),
            Self::TransactionsRequest(msg) => msg.author(),
            Self::PrevotesRequest(msg) => msg.author(),
            Self::PeersRequest(msg) => msg.author(),
            Self::BlockRequest(msg) => msg.author(),
            Self::PoolTransactionsRequest(msg) => msg.author(),
        }
    }
}

impl Consensus {
    /// Returns author public key of the message sender.
    pub fn author(&self) -> PublicKey {
        match self {
            Self::Propose(msg) => msg.author(),
            Self::Prevote(msg) => msg.author(),
            Self::Precommit(msg) => msg.author(),
        }
    }

    /// Returns validator id of the message sender.
    pub fn validator(&self) -> ValidatorId {
        match self {
            Self::Propose(msg) => msg.payload().validator,
            Self::Prevote(msg) => msg.payload().validator,
            Self::Precommit(msg) => msg.payload().validator,
        }
    }

    /// Returns the epoch the message belongs tp.
    pub fn epoch(&self) -> Height {
        match self {
            Self::Propose(msg) => msg.payload().epoch,
            Self::Prevote(msg) => msg.payload().epoch,
            Self::Precommit(msg) => msg.payload().epoch,
        }
    }

    /// Returns round of the message.
    pub fn round(&self) -> Round {
        match self {
            Self::Propose(msg) => msg.payload().round,
            Self::Prevote(msg) => msg.payload().round,
            Self::Precommit(msg) => msg.payload().round,
        }
    }
}

impl BinaryValue for Message {
    fn to_bytes(&self) -> Vec<u8> {
        self.as_raw().to_bytes()
    }

    fn from_bytes(value: Cow<'_, [u8]>) -> anyhow::Result<Self> {
        let message = SignedMessage::from_bytes(value)?;
        Self::from_signed(message)
    }
}

impl ObjectHash for Message {
    fn object_hash(&self) -> Hash {
        self.as_raw().object_hash()
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use exonum::{
        blockchain::{AdditionalHeaders, Block, BlockProof},
        crypto::{self, KeyPair},
        merkledb::ObjectHash,
    };
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_verified_from_signed_correct_signature() {
        let keypair = KeyPair::random();

        let msg = Status {
            epoch: Height(0),
            blockchain_height: Height(0),
            last_hash: Hash::zero(),
            pool_size: 0,
        };
        let protocol_message = ExonumMessage::from(msg.clone());
        let signed = SignedMessage::new(
            protocol_message.clone(),
            keypair.public_key(),
            keypair.secret_key(),
        );

        let verified_protocol = signed.clone().into_verified::<ExonumMessage>().unwrap();
        assert_eq!(*verified_protocol.payload(), protocol_message);
        let verified_status = signed.clone().into_verified::<Status>().unwrap();
        assert_eq!(*verified_status.payload(), msg);

        // Wrong variant
        let err = signed.into_verified::<Precommit>().unwrap_err();
        assert_eq!(err.to_string(), "Failed to decode message from payload.");
    }

    #[test]
    fn test_verified_from_signed_incorrect_signature() {
        let keypair = KeyPair::random();

        let msg = Status {
            epoch: Height(0),
            blockchain_height: Height(0),
            last_hash: Hash::zero(),
            pool_size: 0,
        };
        let protocol_message = ExonumMessage::from(msg);
        let mut signed =
            SignedMessage::new(protocol_message, keypair.public_key(), keypair.secret_key());
        // Update author
        signed.author = KeyPair::random().public_key();
        let err = signed.into_verified::<ExonumMessage>().unwrap_err();
        assert_eq!(err.to_string(), "Failed to verify signature.");
    }

    #[test]
    fn test_verified_status_binary_value() {
        let keypair = KeyPair::random();

        let msg = Verified::from_value(
            Status {
                epoch: Height(0),
                blockchain_height: Height(0),
                last_hash: Hash::zero(),
                pool_size: 0,
            },
            keypair.public_key(),
            keypair.secret_key(),
        );
        assert_eq!(msg.object_hash(), msg.as_raw().object_hash());

        let bytes = msg.to_bytes();
        let msg2 = Verified::<Status>::from_bytes(bytes.into()).unwrap();
        assert_eq!(msg, msg2);
    }

    #[test]
    fn test_tx_response_empty_size() {
        let keys = KeyPair::random();
        let msg = TransactionsResponse::new(keys.public_key(), vec![]);
        let msg = Verified::from_value(msg, keys.public_key(), keys.secret_key());
        assert_eq!(TX_RES_EMPTY_SIZE, msg.into_bytes().len())
    }

    #[test]
    fn test_tx_response_with_txs_size() {
        let keys = KeyPair::random();
        let txs = vec![
            vec![1_u8; 8],
            vec![2_u8; 16],
            vec![3_u8; 64],
            vec![4_u8; 256],
            vec![5_u8; 4096],
        ];
        let txs_size = txs.iter().fold(0, |acc, tx| acc + tx.len());
        let pb_max_overhead = TX_RES_PB_OVERHEAD_PAYLOAD * txs.len();

        let msg = TransactionsResponse::new(keys.public_key(), txs);
        let msg = Verified::from_value(msg, keys.public_key(), keys.secret_key());
        assert!(TX_RES_EMPTY_SIZE + txs_size + pb_max_overhead >= msg.into_bytes().len())
    }

    #[test]
    fn test_block() {
        let keys = KeyPair::random();
        let ts = Utc::now();
        let txs = [2];
        let tx_count = txs.len() as u32;

        let content = Block {
            height: Height(500),
            tx_count,
            prev_hash: crypto::hash(&[1]),
            tx_hash: crypto::hash(&txs),
            state_hash: crypto::hash(&[3]),
            error_hash: crypto::hash(&[4]),
            additional_headers: AdditionalHeaders::new(),
        };

        let precommits = vec![
            Verified::from_value(
                Precommit::new(
                    ValidatorId(123),
                    Height(15),
                    Round(25),
                    crypto::hash(&[1, 2, 3]),
                    crypto::hash(&[3, 2, 1]),
                    ts,
                ),
                keys.public_key(),
                keys.secret_key(),
            ),
            Verified::from_value(
                Precommit::new(
                    ValidatorId(13),
                    Height(25),
                    Round(35),
                    crypto::hash(&[4, 2, 3]),
                    crypto::hash(&[3, 3, 1]),
                    ts,
                ),
                keys.public_key(),
                keys.secret_key(),
            ),
            Verified::from_value(
                Precommit::new(
                    ValidatorId(323),
                    Height(15),
                    Round(25),
                    crypto::hash(&[1, 1, 3]),
                    crypto::hash(&[5, 2, 1]),
                    ts,
                ),
                keys.public_key(),
                keys.secret_key(),
            ),
        ];
        let transactions = [
            Verified::from_value(
                Status::new(Height(2), Height(2), crypto::hash(&[]), 0),
                keys.public_key(),
                keys.secret_key(),
            ),
            Verified::from_value(
                Status::new(Height(4), Height(4), crypto::hash(&[2]), 0),
                keys.public_key(),
                keys.secret_key(),
            ),
            Verified::from_value(
                Status::new(Height(7), Height(7), crypto::hash(&[3]), 0),
                keys.public_key(),
                keys.secret_key(),
            ),
        ]
        .iter()
        .map(ObjectHash::object_hash)
        .collect::<Vec<_>>();

        let precommits_buf: Vec<_> = precommits.iter().map(BinaryValue::to_bytes).collect();
        let block = Verified::from_value(
            BlockResponse::new(
                keys.public_key(),
                content.clone(),
                precommits_buf.clone(),
                transactions.iter().cloned(),
            ),
            keys.public_key(),
            keys.secret_key(),
        );

        assert_eq!(block.author(), keys.public_key());
        assert_eq!(block.payload().to, keys.public_key());
        assert_eq!(block.payload().block, content);
        assert_eq!(block.payload().precommits, precommits_buf);
        assert_eq!(block.payload().transactions, transactions);

        let block2: Verified<BlockResponse> = SignedMessage::from_bytes(block.to_bytes().into())
            .unwrap()
            .into_verified()
            .unwrap();

        assert_eq!(block2.author(), keys.public_key());
        assert_eq!(block2.payload().to, keys.public_key());
        assert_eq!(block2.payload().block, content);
        assert_eq!(block2.payload().precommits, precommits_buf);
        assert_eq!(block2.payload().transactions, transactions);
        let block_proof = BlockProof::new(content, precommits);
        let json_str = serde_json::to_string(&block_proof).unwrap();
        let block_proof_1: BlockProof = serde_json::from_str(&json_str).unwrap();
        assert_eq!(block_proof, block_proof_1);
    }
}
