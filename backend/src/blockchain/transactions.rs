use exonum::storage::Fork;
use exonum::blockchain::Transaction;
use exonum::messages::Message;

use blockchain::ToHash;
use blockchain::dto::{TxUpdateUser, TxPayment, TxTimestamp, TimestampEntry};
use blockchain::schema::Schema;

impl Transaction for TxUpdateUser {
    fn verify(&self) -> bool {
        self.verify_signature(self.pub_key())
    }

    fn execute(&self, view: &mut Fork) {
        let mut schema = Schema::new(view);
        let user = self.content();
        // Checks that proper key used for user info modification.
        let user_id_hash = user.id().to_hash();
        if let Some(entry) = schema.users().get(&user_id_hash) {
            if entry.info().pub_key() != self.pub_key() {
                // Only owner can change user information.
                return;
            }
        }
        schema.add_user(user_id_hash, user);
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
}

impl Transaction for TxTimestamp {
    fn verify(&self) -> bool {
        self.verify_signature(self.pub_key())
    }

    fn execute(&self, view: &mut Fork) {
        let mut schema = Schema::new(view);

        let key_is_suitable = {
            let user_id_hash = self.content().user_id().to_hash();
            if let Some(entry) = schema.users().get(&user_id_hash) {
                debug!(
                    "User key is not same, actual={:?}, expected={:?}",
                    self.pub_key(),
                    entry.info().pub_key()
                );
                entry.info().pub_key() == self.pub_key()
            } else {
                debug!("User not found {}", self.content().user_id());
                false
            }
        };

        if key_is_suitable {
            trace!("Timestamp added: {:?}", self);
            let entry = TimestampEntry::new(self.content(), &self.hash());
            schema.add_timestamp(entry);
        } else {
            debug!("Key is not suitable");
        }
    }
}
