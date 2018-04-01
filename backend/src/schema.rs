use exonum::storage::{Snapshot, ProofMapIndex, ProofListIndex, Fork};
use exonum::crypto::{Hash, PublicKey};
use exonum::blockchain::gen_prefix;

use std::fmt;

use wallet::Wallet;

/// Represents transaction. If `execution_status` equals to `true`, then the transaction
/// was successful.
encoding_struct! {
    struct MetaRecord {
        transaction_hash:    &Hash,
        execution_status:    bool,
    }
}

/// Database schema for the cryptocurrency.
pub struct CurrencySchema<T> {
    view: T,
}

impl<T> AsMut<T> for CurrencySchema<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.view
    }
}

impl<T> fmt::Debug for CurrencySchema<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "CurrencySchema {{}}")
    }
}

impl<T> CurrencySchema<T>
where
    T: AsRef<Snapshot>,
{
    /// Constructs schema from the database view.
    pub fn new(view: T) -> Self {
        CurrencySchema { view }
    }

    /// Returns `MerklePatriciaTable` with wallets.
    pub fn wallets(&self) -> ProofMapIndex<&T, PublicKey, Wallet> {
        ProofMapIndex::new("cryptocurrency.wallets", &self.view)
    }

    /// Returns history of the wallet with the given public key.
    pub fn wallet_history(&self, public_key: &PublicKey) -> ProofListIndex<&T, MetaRecord> {
        ProofListIndex::with_prefix(
            "cryptocurrency.wallet_history",
            gen_prefix(public_key),
            &self.view,
        )
    }

    /// Returns wallet for the given public key.
    pub fn wallet(&self, pub_key: &PublicKey) -> Option<Wallet> {
        self.wallets().get(pub_key)
    }

    /// Returns database state hash.
    pub fn state_hash(&self) -> Vec<Hash> {
        vec![self.wallets().root_hash()]
    }
}

/// Implementation of mutable methods.
impl<'a> CurrencySchema<&'a mut Fork> {
    /// Returns mutable `MerklePatriciaTable` with wallets.
    pub fn wallets_mut(&mut self) -> ProofMapIndex<&mut Fork, PublicKey, Wallet> {
        ProofMapIndex::new("cryptocurrency.wallets", &mut self.view)
    }

    /// Returns history for the wallet by the given public key.
    pub fn wallet_history_mut(
        &mut self,
        public_key: &PublicKey,
    ) -> ProofListIndex<&mut Fork, MetaRecord> {
        ProofListIndex::with_prefix(
            "cryptocurrency.wallet_history",
            gen_prefix(public_key),
            &mut self.view,
        )
    }

    /// Appends transaction record to the wallet with the given public key.
    fn append_history(&mut self, key: &PublicKey, record: MetaRecord) {
        let wallet = {
            let wallet = self.wallet(key).unwrap();
            let mut history = self.wallet_history_mut(key);
            history.push(record);
            wallet.grow_length_set_history_hash(&history.root_hash())
        };
        self.wallets_mut().put(key, wallet);
    }

    /// Appends record with `successful` status to the wallet history.
    pub fn append_success(&mut self, key: &PublicKey, hash: &Hash) {
        let record = MetaRecord::new(hash, true);
        self.append_history(key, record);
    }

    /// Appends record with `unsuccessful` status to the wallet history.
    pub fn append_failure(&mut self, key: &PublicKey, hash: &Hash) {
        let record = MetaRecord::new(hash, false);
        self.append_history(key, record);
    }
}
