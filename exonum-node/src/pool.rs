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
//! To customize block proposals, you should supply a [`ManagePool`] implementation
//! to the [`NodeBuilder`]:
//!
//! ```
//! # use exonum::{keys::Keys, merkledb::TemporaryDB};
//! # use exonum_node::{generate_testnet_config, NodeBuilder, NodeConfig};
//! use exonum_node::pool::{SkipEmptyBlocks, StandardPoolManager};
//!
//! # async fn not_run() -> anyhow::Result<()> {
//! # let (node_config, keys) = generate_testnet_config(1, 2_000).pop().unwrap();
//! let node_config: NodeConfig = // ...
//! #    node_config;
//! let node_keys: Keys = // ...
//! #    keys;
//! let database = TemporaryDB::new();
//! let pool_manager = SkipEmptyBlocks::new(StandardPoolManager::default());
//! let node = NodeBuilder::new(database, node_config, node_keys)
//!     .with_pool_manager(pool_manager)
//!     // specify other node params...
//!     .build();
//! node.run().await?;
//! # Ok(())
//! # }
//! ```
//!
//! [`ManagePool`]: trait.ManagePool.html
//! [`NodeBuilder`]: ../struct.NodeBuilder.html#method.with_pool_manager
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

/// Transaction pool management, responsible for proposing new blocks and garbage-collecting
/// transactions in the memory pool.
///
/// # Proposing Blocks
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
/// # Removing Transactions
///
/// The trait implementation can also be used to garbage-collect transactions once a new block
/// or block skip has been accepted by the node. Note that this is the only time when such garbage
/// collection is safe. Indeed, it cannot be safely performed if node has voted for
/// at least one block proposal at the current epoch, since in this case some of removed transactions
/// may belong to this proposal(s). Removing such a transaction from the node pool may result
/// in consensus stalling.
///
/// [`ProposeTemplate::Skip`]: enum.ProposeTemplate.html#variant.Skip
pub trait ManagePool: Send {
    /// Creates a block proposal based on the transaction pool and block creation params.
    fn propose_block(&mut self, pool: Pool<'_>, params: ProposeParams<'_>) -> ProposeTemplate;

    /// Indicates transactions for removal from the pool of unconfirmed transactions.
    ///
    /// This method is called from the commit handler of the block.
    fn remove_transactions(&mut self, pool: Pool<'_>, snapshot: &dyn Snapshot) -> Vec<Hash>;
}

impl fmt::Debug for dyn ManagePool {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("ProposeBlock").finish()
    }
}

impl ManagePool for Box<dyn ManagePool> {
    fn propose_block(&mut self, pool: Pool<'_>, params: ProposeParams<'_>) -> ProposeTemplate {
        (**self).propose_block(pool, params)
    }

    fn remove_transactions(&mut self, pool: Pool<'_>, snapshot: &dyn Snapshot) -> Vec<Hash> {
        (**self).remove_transactions(pool, snapshot)
    }
}

/// Standard pool manager used by the nodes if no other manager is specified.
///
/// The manager will propose correct transactions in no particular order. It will also remove
/// incorrect transactions from the pool, unless this setting is switched off by using
/// [`with_removal_limit`]`(0)`.
///
/// [`with_removal_limit`]: #method.with_removal_limit
#[derive(Debug, Clone)]
pub struct StandardPoolManager {
    removal_limit: Option<usize>,
}

impl Default for StandardPoolManager {
    fn default() -> Self {
        Self {
            // FIXME: What's the appropriate default value?
            removal_limit: Some(100),
        }
    }
}

impl StandardPoolManager {
    /// Creates a proposer with the specified limit on transactions considered for removal
    /// each time a block is accepted. `None` signifies no limit. Use `0` to switch off transaction
    /// removal logic altogether; with this setting, the node will never remove incorrect transactions
    /// from the pool.
    ///
    /// # Performance notes
    ///
    /// The higher the limit, the
    ///
    /// # Examples
    ///
    /// ```
    /// # use exonum_node::pool::StandardPoolManager;
    /// // Manager that considers no more than 500 transactions for removal
    /// // after a block is accepted.
    /// let manager = StandardPoolManager::with_removal_limit(500);
    /// ```
    pub fn with_removal_limit(removal_limit: impl Into<Option<usize>>) -> Self {
        Self {
            removal_limit: removal_limit.into(),
        }
    }
}

impl ManagePool for StandardPoolManager {
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

    fn remove_transactions(&mut self, pool: Pool<'_>, snapshot: &dyn Snapshot) -> Vec<Hash> {
        let tx_limit = self.removal_limit.unwrap_or_else(usize::max_value);
        pool.transactions()
            .take(tx_limit)
            .filter_map(|(tx_hash, tx)| {
                if let Err(e) = Blockchain::check_tx(snapshot, tx.as_ref()) {
                    log::trace!(
                        "Removing transaction {:?} from pool, since it is incorrect: {}",
                        tx_hash,
                        e
                    );
                    Some(tx_hash)
                } else {
                    None
                }
            })
            .collect()
    }
}

/// Pool manager that skips a block if there are no uncommitted transactions returned by the
/// wrapped manager. The `remove_transactions` method is relayed to the wrapped manager.
#[derive(Debug, Clone, Default)]
pub struct SkipEmptyBlocks<T> {
    inner: T,
}

impl<T: ManagePool> SkipEmptyBlocks<T> {
    /// Creates a new wrapper.
    pub fn new(inner: T) -> Self {
        Self { inner }
    }
}

impl<T: ManagePool> ManagePool for SkipEmptyBlocks<T> {
    fn propose_block(&mut self, pool: Pool<'_>, params: ProposeParams<'_>) -> ProposeTemplate {
        match self.inner.propose_block(pool, params) {
            ProposeTemplate::Ordinary { tx_hashes } if tx_hashes.is_empty() => {
                ProposeTemplate::Skip
            }
            other => other,
        }
    }

    fn remove_transactions(&mut self, pool: Pool<'_>, snapshot: &dyn Snapshot) -> Vec<Hash> {
        self.inner.remove_transactions(pool, snapshot)
    }
}
