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

//! Types used in WebSocket communication with the explorer service.

use chrono::{DateTime, Utc};
use exonum::{
    blockchain::{Block, Schema, TxLocation},
    crypto::Hash,
    merkledb::{access::Access, ListProof},
    runtime::{ExecutionStatus, InstanceId, MethodId},
};
use serde_derive::{Deserialize, Serialize};

use std::fmt;

use super::TransactionHex;
use crate::median_precommits_time;

/// Messages proactively sent by WebSocket clients to the server.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum IncomingMessage {
    /// Set subscription for websocket connection.
    SetSubscriptions(Vec<SubscriptionType>),
    /// Send transaction to the blockchain.
    Transaction(TransactionHex),
}

/// Subscription type for new blocks or committed transactions.
#[derive(Debug, PartialEq, Eq, Hash, Clone, PartialOrd, Ord)]
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SubscriptionType {
    /// Subscription to nothing.
    None,
    /// Subscription to new blocks.
    Blocks,
    /// Subscription to committed transactions.
    Transactions {
        /// Optional filter for the subscription.
        filter: Option<TransactionFilter>,
    },
}

/// Filter for transactions by service instance and (optionally) method identifier
/// within the service.
#[derive(Debug, PartialEq, Eq, Hash, Clone, PartialOrd, Ord)]
#[derive(Serialize, Deserialize)]
pub struct TransactionFilter {
    /// ID of the service.
    pub instance_id: InstanceId,
    /// Optional ID of a method within the service. If not set, transactions belonging
    /// to all service methods will be sent.
    pub method_id: Option<MethodId>,
}

impl TransactionFilter {
    /// Creates a new transaction filter.
    pub fn new(instance_id: InstanceId, method_id: Option<MethodId>) -> Self {
        Self {
            instance_id,
            method_id,
        }
    }
}

/// Response to a WebSocket client. Roughly equivalent to `Result<T, String>`.
#[serde(tag = "result", rename_all = "snake_case")]
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum Response<T> {
    /// Successful response.
    Success {
        /// Payload attached to the response.
        response: T,
    },
    /// Response carrying an error.
    Error {
        /// Error description.
        description: String,
    },
}

impl<T> Response<T> {
    /// Creates a response with the specified value.
    pub fn success(value: T) -> Self {
        Response::Success { response: value }
    }

    /// Creates an erroneous response.
    pub fn error(description: impl fmt::Display) -> Self {
        Response::Error {
            description: description.to_string(),
        }
    }

    /// Converts response into a `Result`.
    pub fn into_result(self) -> Result<T, String> {
        match self {
            Response::Success { response } => Ok(response),
            Response::Error { description } => Err(description),
        }
    }
}

impl<T> From<Result<T, String>> for Response<T> {
    fn from(res: Result<T, String>) -> Self {
        match res {
            Ok(value) => Self::success(value),
            Err(description) => Response::Error { description },
        }
    }
}

/// Summary about a particular transaction in the blockchain. Does not include transaction content.
#[derive(Debug, Serialize, Deserialize)]
pub struct CommittedTransactionSummary {
    /// Transaction identifier.
    pub tx_hash: Hash,
    /// ID of service.
    pub instance_id: InstanceId,
    /// ID of the method within service.
    pub method_id: MethodId,
    /// Result of transaction execution.
    pub status: ExecutionStatus,
    /// Transaction location in the blockchain.
    pub location: TxLocation,
    /// Proof of existence.
    pub location_proof: ListProof<Hash>,
    /// Approximate finalization time.
    pub time: DateTime<Utc>,
}

impl CommittedTransactionSummary {
    /// Constructs a transaction summary from the core schema.
    pub fn new(schema: &Schema<impl Access>, tx_hash: &Hash) -> Option<Self> {
        let tx = schema.transactions().get(tx_hash)?;
        let tx = tx.payload();
        let instance_id = tx.call_info.instance_id;
        let method_id = tx.call_info.method_id;
        let location = schema.transactions_locations().get(tx_hash)?;
        let tx_result = schema.transaction_result(location)?;
        let location_proof = schema
            .block_transactions(location.block_height())
            .get_proof(location.position_in_block().into());
        let time = median_precommits_time(
            &schema
                .block_and_precommits(location.block_height())
                .unwrap()
                .precommits,
        );
        Some(Self {
            tx_hash: *tx_hash,
            instance_id,
            method_id,
            status: ExecutionStatus(tx_result),
            location,
            location_proof,
            time,
        })
    }
}

/// Notification message passed to WebSocket clients.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Notification {
    /// Notification about new block.
    Block(Block),
    /// Notification about new transaction.
    Transaction(CommittedTransactionSummary),
}
