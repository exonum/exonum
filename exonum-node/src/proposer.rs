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

//! Utilities allowing to customize proposal creation logic for an Exonum node.
//!
//! To customize block proposals, you should supply a [`ProposeBlock`] implementation
//! to the [`NodeBuilder`]:
//!
//! ```
//! # use exonum::{keys::Keys, merkledb::TemporaryDB};
//! # use exonum_node::{generate_testnet_config, NodeBuilder, NodeConfig};
//! use exonum_node::proposer::SkipEmptyBlocks;
//!
//! # async fn not_run() -> anyhow::Result<()> {
//! # let (node_config, keys) = generate_testnet_config(1, 2_000).pop().unwrap();
//! let node_config: NodeConfig = // ...
//! #    node_config;
//! let node_keys: Keys = // ...
//! #    keys;
//! let database = TemporaryDB::new();
//! let node = NodeBuilder::new(database, node_config, node_keys)
//!     .with_block_proposer(SkipEmptyBlocks)
//!     // specify other node params...
//!     .build();
//! node.run().await?;
//! # Ok(())
//! # }
//! ```
//!
//! [`ProposeBlock`]: trait.ProposeBlock.html
//! [`NodeBuilder`]: ../struct.NodeBuilder.html#method.with_block_proposer
//!
//! # Stability
//!
//! The contents of this module is considered unstable and experimental. It may change in any way
//! between `exonum-node` releases.
//!
//! # Safety
//!
//! **USING CUSTOM PROPOSER LOGIC CAN LEAD TO CONSENSUS HANG-UP AND OTHER ADVERSE EFFECTS.**
//! Consensus safety and liveness properties proven in the [Exonum white paper]
//! **DO NOT HOLD** for arbitrary proposal creation logic.
//!
//! [Exonum white paper]: https://bitfury.com/content/downloads/wp_consensus_181227.pdf

use exonum::{
    blockchain::{Blockchain, ConsensusConfig, PersistentPool, TransactionCache},
    crypto::Hash,
    helpers::{Height, Round},
    merkledb::Snapshot,
    messages::{AnyTx, Verified},
};

use std::{collections::BTreeMap, fmt};

use crate::State;

/// Type alias for the persistent pool.
pub type Pool<'a> = PersistentPool<'a, BTreeMap<Hash, Verified<AnyTx>>, &'a dyn Snapshot>;

/// Block proposal parameters supplied to the proposer from the node.
#[derive(Debug)]
pub struct ProposeParams<'a> {
    consensus_config: ConsensusConfig,
    height: Height,
    round: Round,
    snapshot: &'a dyn Snapshot,
}

impl<'a> ProposeParams<'a> {
    pub(crate) fn new(state: &State, snapshot: &'a dyn Snapshot) -> Self {
        Self {
            consensus_config: state.consensus_config().to_owned(),
            height: state.epoch(),
            round: state.round(),
            snapshot,
        }
    }

    /// Current consensus configuration.
    pub fn consensus_config(&self) -> &ConsensusConfig {
        &self.consensus_config
    }

    /// Current blockchain height.
    pub fn height(&self) -> Height {
        self.height
    }

    /// Current consensus round.
    pub fn round(&self) -> Round {
        self.round
    }

    /// Returns the snapshot of the current blockchain state.
    pub fn snapshot(&self) -> &'a dyn Snapshot {
        self.snapshot
    }
}

/// Propose template returned by the proposal creator.
///
/// This type is effectively the owned version of `BlockContents` from `exonum::blockchain`.
/// See its documentation for more details about supported block types.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProposeTemplate {
    /// Ordinary proposal with a list of transaction hashes.
    ///
    /// It is the responsibility of the `ProposeBlock` implementation to assure that:
    ///
    /// - Transactions with the specified hashes are known to the node
    /// - Transaction hashes do not repeat
    /// - The amount of hashes is not higher than the constraints in the `ConsensusConfig`
    /// - Transactions with the specified hashes are correct (i.e., pass `Blockchain::check_tx`).
    Ordinary {
        /// Hashes of the transactions in the proposal.
        tx_hashes: Vec<Hash>,
    },

    /// Skip block for this epoch of the consensus algorithm.
    Skip,
}

impl ProposeTemplate {
    /// Creates a new `Propose` template.
    pub fn ordinary(tx_hashes: impl IntoIterator<Item = Hash>) -> Self {
        Self::Ordinary {
            tx_hashes: tx_hashes.into_iter().collect(),
        }
    }
}

/// Proposal creation logic.
///
/// An implementation of this trait can be supplied to a node to change how the node will
/// form block proposals. Some use cases for this functionality are as follows:
///
/// - Whitelist or blacklist public keys authorizing transactions
/// - Whitelist, blacklist, otherwise filter and/or prioritize transactions by the called
///   service + method combination. As an example, this may be used to implement
///   "crypto-economics", in which all transactions need to pay a fee, solve a proof-of-work, etc.
/// - Restrict the number of transactions and their size in a more flexible manner than
///   supported on the consensus level
/// - Skip block creation if certain conditions are met by returning [`ProposeTemplate::Skip`].
///
/// The block proposer is set for each node individually and does not necessarily need to agree
/// among nodes. At the same time, some proposer implementations (such as whitelisting or blacklisting)
/// do not work effectively unless adopted by all validator nodes.
///
/// [`ProposeTemplate::Skip`]: enum.ProposeTemplate.html#variant.Skip
pub trait ProposeBlock: Send {
    /// Creates a block proposal based on the transaction pool and block creation params.
    fn propose_block(&mut self, pool: Pool<'_>, params: ProposeParams<'_>) -> ProposeTemplate;
}

impl fmt::Debug for dyn ProposeBlock {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("ProposeBlock").finish()
    }
}

impl ProposeBlock for Box<dyn ProposeBlock> {
    fn propose_block(&mut self, pool: Pool<'_>, params: ProposeParams<'_>) -> ProposeTemplate {
        (**self).propose_block(pool, params)
    }
}

/// Standard block proposer used by the nodes if no other proposer is specified.
#[derive(Debug, Clone)]
pub struct StandardProposer;

impl ProposeBlock for StandardProposer {
    fn propose_block(&mut self, pool: Pool<'_>, params: ProposeParams<'_>) -> ProposeTemplate {
        let max_transactions = params.consensus_config.txs_block_limit;
        let snapshot = params.snapshot();

        let tx_hashes = pool
            .transactions()
            .filter_map(|(tx_hash, tx)| {
                // TODO: this is wildly inefficient.
                // It should be easy to cache tx status within single height; however,
                // spanning cache across multiple heights would be significantly harder.
                if Blockchain::check_tx(snapshot, tx.as_ref()).is_ok() {
                    Some(tx_hash)
                } else {
                    None
                }
            })
            .take(max_transactions as usize);

        ProposeTemplate::ordinary(tx_hashes)
    }
}

/// Block proposer that skips a block if there are no uncommitted transactions.
#[derive(Debug, Clone)]
pub struct SkipEmptyBlocks;

impl ProposeBlock for SkipEmptyBlocks {
    fn propose_block(&mut self, pool: Pool<'_>, params: ProposeParams<'_>) -> ProposeTemplate {
        match StandardProposer.propose_block(pool, params) {
            ProposeTemplate::Ordinary { tx_hashes } if tx_hashes.is_empty() => {
                ProposeTemplate::Skip
            }
            other => other,
        }
    }
}
