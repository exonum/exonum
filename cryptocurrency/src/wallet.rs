
use exonum::messages::Field;
use exonum::crypto::{PublicKey, Hash, hash};
use exonum::storage::StorageValue;

pub const WALLET_SIZE: usize = 80;

pub type WalletId = u64;

// Реальная структура кошелька
// struct Wallet {
//     pub_key: PublicKey,
//     balance: u64,
//     history_hash: Hash
//     name: String,
// }
#[derive(Debug)]
pub struct Wallet {
    // Публичный ключ владельца кошелька
    // Текущий баланс
    // Имя владельца кошелька
    raw: Vec<u8>,
}

impl Wallet {
    pub fn new<S: AsRef<str>>(public_key: &PublicKey, name: S, amount: i64, history_hash: &Hash) -> Wallet {
        let mut wallet = Wallet { raw: vec![0; WALLET_SIZE] };

        Field::write(&public_key, &mut wallet.raw, 0, 32);
        Field::write(&amount, &mut wallet.raw, 32, 40);
        Field::write(&history_hash, &mut wallet.raw, 40, 72);
        Field::write(&name.as_ref(), &mut wallet.raw, 72, 80);
        wallet
    }

    pub fn from_raw(raw: Vec<u8>) -> Wallet {
        // TODO: error instead of panic?
        // assert_eq!(raw.len(), WALLET_SIZE);
        Wallet { raw: raw }
    }

    pub fn pub_key(&self) -> &PublicKey {
        Field::read(&self.raw, 0, 32)
    }

    pub fn balance(&self) -> i64 {
        Field::read(&self.raw, 32, 40)
    }

    pub fn history_hash(&self) -> &Hash {
        Field::read(&self.raw, 40, 72)
    }

    pub fn name(&self) -> &str {
        Field::read(&self.raw, 72, 80)
    }

    pub fn hash(&self) -> Hash {
        hash(&self.raw)
    }

    pub fn set_balance(&mut self, balance: i64) {
        Field::write(&balance, &mut self.raw, 32, 40);
    }

    pub fn set_history_hash(&mut self, hash: &Hash) {
        Field::write(&hash, &mut self.raw, 40, 72);
    }

    pub fn transfer_to(&mut self, other: &mut Wallet, amount: i64) {
        let self_amount = self.balance() - amount;
        let other_amount = other.balance() + amount;
        self.set_balance(self_amount);
        other.set_balance(other_amount);
    }
}

impl StorageValue for Wallet {
    fn serialize(self) -> Vec<u8> {
        self.raw
    }

    fn deserialize(v: Vec<u8>) -> Self {
        Wallet::from_raw(v)
    }

    fn hash(&self) -> Hash {
        Wallet::hash(self)
    }
}

#[test]
fn test_wallet() {
    let hash = Hash([2; 32]);
    let name = "foobar abacaba";
    let pub_key = PublicKey::from_slice([1u8; 32].as_ref()).unwrap();
    let wallet = Wallet::new(&pub_key, name, -100500, &hash);

    assert_eq!(wallet.pub_key(), &pub_key);
    assert_eq!(wallet.name(), name);
    assert_eq!(wallet.balance(), -100500);
    assert_eq!(wallet.history_hash(), &hash);
}

#[test]
fn test_amount_transfer() {
    let hash = Hash([5; 32]);
    let pub_key = PublicKey::from_slice([1u8; 32].as_ref()).unwrap();
    let mut a = Wallet::new(&pub_key, "a", 100, &hash);
    let mut b = Wallet::new(&pub_key, "b", 0, &hash);
    a.transfer_to(&mut b, 50);

    assert_eq!(a.balance(), 50);
    assert_eq!(b.balance(), 50);
}
