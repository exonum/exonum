pub mod dto;

use serde_json;
use serde_json::Value;

use exonum::crypto::Hash;
use exonum::storage::{Snapshot, Fork};
use exonum::blockchain::{Transaction, gen_prefix};
use exonum::storage::ProofMapIndex;

use TIMESTAMPING_SERVICE_ID;

pub const TIMESTAMPING_TX_ID: u16 = 0;

message! {
    struct TimestampTx {
        const TYPE = TIMESTAMPING_SERVICE_ID;
        const ID = TIMESTAMPING_TX_ID;
        const SIZE = 8;

        field content:        Content     [00 => 08]
    }
}

encoding_struct! {
    struct Content {
        const SIZE = 40;

        field description:        &str        [00 => 08]
        field data_hash:          &Hash       [08 => 40]
    }
}

pub struct TimestampingSchema<T> {
    view: T,
}

impl<T> TimestampingSchema<T>
where
    T: AsRef<Snapshot>,
{
    pub fn new(snapshot: T) -> TimestampingSchema<T> {
        TimestampingSchema { view: snapshot }
    }

    pub fn contents(&self) -> ProofMapIndex<&T, Hash, Content> {
        let prefix = gen_prefix(TIMESTAMPING_SERVICE_ID, 0, &());
        ProofMapIndex::new(prefix, &self.view)
    }

    pub fn state_hash(&self) -> Vec<Hash> {
        vec![self.contents().root_hash()]
    }
}

impl<'a> TimestampingSchema<&'a mut Fork> {
    pub fn contents_mut(&mut self) -> ProofMapIndex<&mut Fork, Hash, Content> {
        let prefix = gen_prefix(TIMESTAMPING_SERVICE_ID, 0, &());
        ProofMapIndex::new(prefix, &mut self.view)
    }
}

impl Transaction for TimestampTx {
    fn verify(&self) -> bool {
        true
    }

    fn execute(&self, fork: &mut Fork) {
        let mut schema = TimestampingSchema::new(fork);
        let content = self.content();
        schema.contents_mut().put(
            content.data_hash(),
            content.clone(),
        )
    }

    fn info(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use exonum::crypto::{gen_keypair, hash};

    use super::{TimestampTx, Content};

    #[test]
    fn test_timestamp_tx() {
        let description = "Test Description";
        let hash = hash(b"js9sdhcsdh32or830ru8043ru-wf9-12u8u3280y8hfwoefnsdljs");
        let (_, sec_key) = gen_keypair();

        let content = Content::new(description, &hash);
        let tx = TimestampTx::new(content.clone(), &sec_key);
        assert_eq!(tx.content(), content);
    }

    #[test]
    fn test_file_content() {
        let description = "Test Description";
        let hash = hash(b"js9sdhcsdh32or830ru8043ru-wf9-12u8u3280y8hfwoefnsdljs");

        let content = Content::new(description, &hash);
        assert_eq!(content.description(), description);
        assert_eq!(content.data_hash(), &hash);
    }
}
