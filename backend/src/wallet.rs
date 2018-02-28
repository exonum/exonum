//! Wallet.

use exonum::encoding::Field;
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
    /// Overwrites balance.
    pub fn set_balance(&mut self, balance: u64) {
        Field::write(&balance, &mut self.raw, 40, 48);
    }

    /// Sets history hash.
    pub fn grow_length_set_history_hash(&mut self, hash: &Hash) {
        Field::write(&hash, &mut self.raw, 56, 88);
        Field::write(&(self.history_len() + 1), &mut self.raw, 48, 56);
    }

    /// Transfers currency from this wallet to `other`. Returns `false` if `self.balance()` is
    /// less then `amount`.
    pub fn transfer_to(&mut self, other: &mut Wallet, amount: u64) -> bool {
        if self.pub_key() == other.pub_key() {
            return false;
        }

        if self.balance() < amount {
            return false;
        }

        let self_amount = self.balance() - amount;
        let other_amount = other.balance() + amount;
        self.set_balance(self_amount);
        other.set_balance(other_amount);
        true
    }
}

#[cfg(test)]
pub fn assert_wallet(
    wallet: &Wallet,
    pub_key: &PublicKey,
    name: &str,
    balance: u64,
    history_len: u64,
    history_hash: &Hash,
) {
    assert_eq!(wallet.pub_key(), pub_key);
    assert_eq!(wallet.name(), name);
    assert_eq!(wallet.balance(), balance);
    assert_eq!(wallet.history_hash(), history_hash);
    assert_eq!(wallet.history_len(), history_len);
}

#[cfg(test)]
mod tests {
    use exonum::storage::StorageValue;
    use super::*;

    #[test]
    fn test_wallet() {
        let hash = Hash::new([2; 32]);
        let name = "foobar abacaba Юникод всяуи";
        let pub_key = PublicKey::from_slice([1u8; 32].as_ref()).unwrap();
        let wallet = Wallet::new(&pub_key, name, 100500, 0, &hash);

        let wallet = wallet.clone();
        assert_wallet(&wallet, &pub_key, name, 100500, 0, &hash);
    }

    #[test]
    fn test_wallet_serde() {
        use serde_json;
        use rand::{thread_rng, Rng};
        use exonum::crypto::{HASH_SIZE, gen_keypair};

        let mut rng = thread_rng();
        let generator = move |_| {
            let string_len = rng.gen_range(20u8, 255u8);
            let mut hash_bytes = [0; HASH_SIZE];

            let (pub_key, _) = gen_keypair();
            let name: String = rng.gen_ascii_chars().take(string_len as usize).collect();
            let balance = rng.next_u64();
            let history_len = rng.next_u64();
            rng.fill_bytes(&mut hash_bytes);
            let hash = Hash::new(hash_bytes);
            Wallet::new(&pub_key, &name, balance, history_len, &hash)
        };
        let wallet_non_ascii = Wallet::new(
            &gen_keypair().0,
            "foobar abacaba Юникод всяуи",
            100500,
            0,
            &Hash::new([2; HASH_SIZE]),
        );
        let mut wallets = (0..50).map(generator).collect::<Vec<_>>();
        wallets.push(wallet_non_ascii);
        for wallet in wallets {
            let json_str = serde_json::to_string(&wallet).unwrap();
            let wallet1: Wallet = serde_json::from_str(&json_str).unwrap();
            assert_eq!(wallet, wallet1);
            trace!(
                "wallet test data: {}",
                serde_json::to_string(&WalletTestData::new(wallet)).unwrap()
            );
        }
    }

    #[test]
    fn test_amount_transfer() {
        let hash = Hash::new([5; 32]);
        let pub_key_1 = PublicKey::from_slice([1u8; 32].as_ref()).unwrap();
        let pub_key_2 = PublicKey::from_slice([2u8; 32].as_ref()).unwrap();
        let mut a = Wallet::new(&pub_key_1, "a", 100, 12, &hash);
        let mut b = Wallet::new(&pub_key_2, "b", 0, 14, &hash);
        a.transfer_to(&mut b, 50);
        a.grow_length_set_history_hash(&hash);
        b.grow_length_set_history_hash(&hash);
        assert_eq!(a.balance(), 50);
        assert_eq!(a.history_len(), 13);
        assert_eq!(b.balance(), 50);
        assert_eq!(b.history_len(), 15);
    }

    #[test]
    fn test_same_wallet_transfer() {
        let hash = Hash::new([5; 32]);
        let pub_key = PublicKey::from_slice([1u8; 32].as_ref()).unwrap();
        let mut a1 = Wallet::new(&pub_key, "a", 100, 12, &hash);
        let mut a2 = Wallet::new(&pub_key, "a", 100, 12, &hash);
        assert_eq!(a1.transfer_to(&mut a2, 50), false);
        assert_eq!(a2.transfer_to(&mut a1, 50), false);
    }

    #[derive(Serialize)]
    struct WalletTestData {
        wallet: Wallet,
        hash: Hash,
        raw: Vec<u8>,
    }

    impl WalletTestData {
        fn new(wallet: Wallet) -> WalletTestData {
            let wallet_hash = wallet.hash();
            let raw = StorageValue::into_bytes(wallet.clone());
            WalletTestData {
                wallet: wallet,
                hash: wallet_hash,
                raw: raw,
            }
        }
    }
}
