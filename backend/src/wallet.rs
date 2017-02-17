
use exonum::messages::Field;
use exonum::crypto::{PublicKey, Hash, hash, ToHex};
use serde::{Serialize, Serializer};

pub type WalletId = u64;

storage_value! {
    Wallet {
        const SIZE = 88;

        pub_key:            &PublicKey  [00 => 32]
        name:               &str        [32 => 40]
        balance:            u64         [40 => 48]
        history_len:        u64         [48 => 56]
        history_hash:       &Hash       [56 => 88]
    }
}

impl Wallet {
    pub fn set_balance(&mut self, balance: u64) {
        Field::write(&balance, &mut self.raw, 40, 48);
    }

    pub fn increase_history_len(&mut self) {
        let old_value = self.history_len();
        let incremented = old_value + 1;
        Field::write(&incremented, &mut self.raw, 48, 56);
    }

    pub fn set_history_hash(&mut self, hash: &Hash) {
        Field::write(&hash, &mut self.raw, 56, 88);
    }

    pub fn transfer_to(&mut self, other: &mut Wallet, amount: u64) {
        let self_amount = self.balance() - amount;
        let other_amount = other.balance() + amount;
        self.set_balance(self_amount);
        other.set_balance(other_amount);
    }
}

impl Serialize for Wallet {
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        let mut state = ser.serialize_struct("wallet", 3)?;
        ser.serialize_struct_elt(&mut state, "balance", self.balance())?;
        ser.serialize_struct_elt(&mut state, "name", self.name())?;
        ser.serialize_struct_elt(&mut state, "history_hash", self.history_hash().to_hex())?;
        ser.serialize_struct_end(state)
    }
}

#[test]
fn test_wallet() {
    let hash = Hash::new([2; 32]);
    let name = "foobar abacaba Юникод всяуи";
    let pub_key = PublicKey::from_slice([1u8; 32].as_ref()).unwrap();
    let wallet = Wallet::new(&pub_key, name, 100500, 0, &hash);

    let wallet = wallet.clone();
    assert_eq!(wallet.pub_key(), &pub_key);
    assert_eq!(wallet.name(), name);
    assert_eq!(wallet.balance(), 100500);
    assert_eq!(wallet.history_hash(), &hash);
    assert_eq!(wallet.history_len(), 0);
}

#[test]
fn test_amount_transfer() {
    let hash = Hash::new([5; 32]);
    let pub_key = PublicKey::from_slice([1u8; 32].as_ref()).unwrap();
    let mut a = Wallet::new(&pub_key, "a", 100, 12, &hash);
    let mut b = Wallet::new(&pub_key, "b", 0, 14, &hash);
    a.transfer_to(&mut b, 50);
    a.increase_history_len();
    b.increase_history_len();
    assert_eq!(a.balance(), 50);
    assert_eq!(a.history_len(), 13);
    assert_eq!(b.balance(), 50);
    assert_eq!(b.history_len(), 15);
}
