use exonum::crypto::Hash;
use exonum::storage::{StorageValue, StorageKey, ProofMapIndex, ProofListIndex, Snapshot, Fork};
use exonum::storage::proof_map_index::ProofMapKey;
use exonum::blockchain::gen_prefix;

use TIMESTAMPING_SERVICE_ID;
use blockchain::dto::{UserInfoEntry, Timestamp, PaymentInfo};

#[derive(Debug)]
pub struct Schema<T> {
    view: T,
}

/// Information schema.
impl<T> Schema<T>
where
    T: AsRef<Snapshot>,
{
    pub fn users(&self) -> ProofMapIndex<&T, &str, UserInfoEntry> {
        let prefix = gen_prefix(TIMESTAMPING_SERVICE_ID, 0, &());
        ProofMapIndex::new(prefix, &self.view)
    }

    pub fn timestamps(&self, user_id: &str) -> ProofListIndex<&T, Timestamp> {
        let prefix = gen_prefix(TIMESTAMPING_SERVICE_ID, 1, &user_id.to_owned());
        ProofListIndex::new(prefix, &self.view)
    }

    pub fn payments(&self, user_id: &str) -> ProofListIndex<&T, PaymentInfo> {
        let prefix = gen_prefix(TIMESTAMPING_SERVICE_ID, 2, &user_id.to_owned());
        ProofListIndex::new(prefix, &self.view)
    }
}

/// Business logic part.
impl<'a> Schema<&'a mut Fork> {
    pub fn users_mut(&mut self) -> ProofMapIndex<&mut Fork, &str, UserInfoEntry> {
        let prefix = gen_prefix(TIMESTAMPING_SERVICE_ID, 0, &());
        ProofMapIndex::new(prefix, &mut self.view)
    }

    pub fn timestamps_mut(&mut self, user_id: &str) -> ProofListIndex<&mut Fork, Timestamp> {
        let prefix = gen_prefix(TIMESTAMPING_SERVICE_ID, 1, &user_id.to_owned());
        ProofListIndex::new(prefix, &mut self.view)
    }

    pub fn payments_mut(&mut self, user_id: &str) -> ProofListIndex<&mut Fork, PaymentInfo> {
        let prefix = gen_prefix(TIMESTAMPING_SERVICE_ID, 2, &user_id.to_owned());
        ProofListIndex::new(prefix, &mut self.view)
    }
}