//! Wallet.

use exonum::crypto::{PublicKey, Hash};

encoding_struct! {
    /// Wallet information stored in the database.
    struct Wallet {
        pub_key:            &PublicKey,
        name:               &str,
        balance:            u64,
        history_len:        u64,
        history_hash:       &Hash,
    }
}

impl Wallet {
    /// Creates new wallet with provided public key, name and balance.
    pub fn create(pub_key: &PublicKey, name: &str, balance: u64) -> Self {
        Wallet::new(pub_key, name, balance, 0, &Hash::zero())
    }

    /// Returns a copy of this wallet with updated balance.
    pub fn set_balance(self, balance: u64) -> Self {
        Self::new(
            self.pub_key(),
            self.name(),
            balance,
            self.history_len(),
            self.history_hash(),
        )
    }

    /// Returns a copy of this wallet with the balance increased by the specified amount.
    pub fn increase(self, amount: u64) -> Self {
        let balance = self.balance() + amount;
        self.set_balance(balance)
    }

    /// Returns a copy of this wallet with the balance decreased by the specified amount.
    pub fn decrease(self, amount: u64) -> Self {
        let balance = self.balance() - amount;
        self.set_balance(balance)
    }

    /// Sets history hash.
    pub fn grow_length_set_history_hash(self, hash: &Hash) -> Self {
        Self::new(
            self.pub_key(),
            self.name(),
            self.balance(),
            self.history_len() + 1,
            hash,
        )
    }
}
