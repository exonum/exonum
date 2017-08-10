use serde_json::{Value, to_value};

use exonum::storage::Fork;
use exonum::blockchain::Transaction;
use exonum::messages::{FromRaw, Message, RawTransaction};
use exonum::crypto::{Hash, PublicKey};
use exonum::encoding::Error as StreamStructError;

use super::dto::{TxUpdateUser, TxPayment, TxTimestamp};
use super::schema::Schema;

impl Transaction for TxUpdateUser {
    fn verify(&self) -> bool {
        self.verify_signature(self.pub_key())
    }

    fn execute(&self, view: &mut Fork) {
        let mut schema = Schema::new(view);
        schema.add_user(self.content());
    }

    fn info(&self) -> Value {
        to_value(self).unwrap()
    }
}

impl Transaction for TxPayment {
    fn verify(&self) -> bool {
        self.verify_signature(self.pub_key())
    }

    fn execute(&self, view: &mut Fork) {
        let mut schema = Schema::new(view);
        schema.add_payment(self.content());
    }

    fn info(&self) -> Value {
        to_value(self).unwrap()
    }
}

impl Transaction for TxTimestamp {
    fn verify(&self) -> bool {
        self.verify_signature(self.pub_key())
    }

    fn execute(&self, view: &mut Fork) {
        let mut schema = Schema::new(view);
        schema.add_timestamp(self.content());
    }

    fn info(&self) -> Value {
        to_value(self).unwrap()
    }
}