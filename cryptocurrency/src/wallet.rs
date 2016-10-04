
use exonum::messages::Field;
use exonum::crypto::{PublicKey, Hash, hash};

pub type WalletId = u64;

storage_value! {
    Wallet {
        const SIZE = 80;

        pub_key:            &PublicKey  [00 => 32]
        name:               &str        [72 => 80]
        balance:            i64         [32 => 40]
        history_hash:       &Hash       [40 => 72]
    }
}

impl Wallet {
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

#[test]
fn test_wallet() {
    let hash = Hash([2; 32]);
    let name = "foobar abacaba Юникод всяуи";
    let pub_key = PublicKey::from_slice([1u8; 32].as_ref()).unwrap();
    let wallet = Wallet::new(&pub_key, name, -100500, &hash);

    let wallet = wallet.clone();
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
