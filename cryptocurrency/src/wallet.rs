
use exonum::messages::{Field};
use exonum::crypto::{PublicKey, Hash, hash};
use exonum::storage::StorageValue;

pub const WALLET_SIZE: usize = 48;

#[derive(Debug)]
pub struct Wallet {
    // Публичный ключ владельца кошелька
    // Цифровой номер ключа
    // Имя владельца кошелька
    raw: Vec<u8>,
}

impl Wallet {
    pub fn new<S: AsRef<str>>(public_key: &PublicKey, code: u64, name: S)
               -> Wallet {
        let mut wallet = Wallet { raw: vec![0; WALLET_SIZE] };

        Field::write(&public_key, &mut wallet.raw, 0, 32);
        Field::write(&code, &mut wallet.raw, 32, 40);
        Field::write(&name.as_ref(), &mut wallet.raw, 40, 48);
        wallet
    }

    pub fn from_raw(raw: Vec<u8>) -> Wallet {
        // TODO: error instead of panic?
        assert_eq!(raw.len(), WALLET_SIZE);
        Wallet { raw: raw }
    }

    pub fn pub_key(&self) -> &PublicKey {
        Field::read(&self.raw, 0, 32)
    }

    pub fn code(&self) -> u64 {
        Field::read(&self.raw, 32, 40)
    }

    pub fn name(&self) -> &str {
        Field::read(&self.raw, 40, 48)
    }

    pub fn hash(&self) -> Hash {
        hash(&self.raw)
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
    let code = 1234;
    let pub_key = PublicKey::from_slice([1u8;32].as_ref()).unwrap();
    let name = "foobar abacaba";
    let wallet = Wallet::new(&pub_key, code, name);

    assert_eq!(wallet.code(), code);
    assert_eq!(wallet.pub_key(), &pub_key);
    assert_eq!(wallet.name(), name);

}
