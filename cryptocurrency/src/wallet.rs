
use exonum::messages::Field;
use exonum::crypto::{PublicKey, Hash, hash};
use exonum::storage::StorageValue;

pub const WALLET_SIZE: usize = 48;

pub type WalletId = u64;

#[derive(Debug)]
pub struct Wallet {
    // Публичный ключ владельца кошелька
    // Текущий баланс
    // Имя владельца кошелька
    raw: Vec<u8>,
}

impl Wallet {
    pub fn new<S: AsRef<str>>(public_key: &PublicKey, name: S, amount: i64) -> Wallet {
        let mut wallet = Wallet { raw: vec![0; WALLET_SIZE] };

        Field::write(&public_key, &mut wallet.raw, 0, 32);
        wallet.set_amount(amount);
        Field::write(&name.as_ref(), &mut wallet.raw, 40, 48);
        wallet
    }

    pub fn from_raw(raw: Vec<u8>) -> Wallet {
        // TODO: error instead of panic?
        //assert_eq!(raw.len(), WALLET_SIZE);
        Wallet { raw: raw }
    }

    pub fn pub_key(&self) -> &PublicKey {
        Field::read(&self.raw, 0, 32)
    }

    pub fn amount(&self) -> i64 {
        Field::read(&self.raw, 32, 40)
    }

    pub fn name(&self) -> &str {
        Field::read(&self.raw, 40, 48)
    }

    pub fn hash(&self) -> Hash {
        hash(&self.raw)
    }

    pub fn transfer_to(&mut self, other: &mut Wallet, amount: i64) {
        let self_amount = self.amount() - amount;
        let other_amount = other.amount() + amount;
        self.set_amount(self_amount);
        other.set_amount(other_amount);
    }

    pub fn set_amount(&mut self, amount: i64) {
        Field::write(&amount, &mut self.raw, 32, 40);
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
    let name = "foobar abacaba";
    let pub_key = PublicKey::from_slice([1u8; 32].as_ref()).unwrap();
    let wallet = Wallet::new(&pub_key, name, -100500);

    assert_eq!(wallet.pub_key(), &pub_key);
    assert_eq!(wallet.name(), name);
    assert_eq!(wallet.amount(), -100500);
}

#[test]
fn test_amount_transfer() {
    let pub_key = PublicKey::from_slice([1u8; 32].as_ref()).unwrap();
    let mut a = Wallet::new(&pub_key, "a", 100);
    let mut b = Wallet::new(&pub_key, "b", 0);
    a.transfer_to(&mut b, 50);

    assert_eq!(a.amount(), 50);
    assert_eq!(b.amount(), 50);
}
