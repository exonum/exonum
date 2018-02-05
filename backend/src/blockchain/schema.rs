use exonum::crypto::{Hash, PublicKey};
use exonum::storage::{MapIndex, ListIndex, ProofMapIndex, ProofListIndex, Snapshot, Fork};
use exonum::blockchain::gen_prefix;

use blockchain::ToHash;
use blockchain::dto::{UserInfoEntry, UserInfo, TimestampEntry, PaymentInfo};

pub const INITIAL_TIMESTAMPS: i64 = 10;

#[derive(Debug)]
pub struct Schema<T> {
    view: T,
}

/// Timestamping information schema.
impl<T> Schema<T> {
    pub fn new(snapshot: T) -> Schema<T> {
        Schema { view: snapshot }
    }

    pub fn into_view(self) -> T {
        self.view
    }
}

impl<T> Schema<T>
where
    T: AsRef<Snapshot>,
{
    pub fn users(&self) -> ProofMapIndex<&T, Hash, UserInfoEntry> {
        ProofMapIndex::new("timestamping.users", &self.view)
    }

    pub fn timestamps(&self) -> ProofMapIndex<&T, Hash, TimestampEntry> {
        ProofMapIndex::new("timestamping.timestamps", &self.view)
    }

    pub fn payments(&self, user_id: &str) -> ProofListIndex<&T, PaymentInfo> {
        let prefix = gen_prefix(&user_id.to_owned());
        ProofListIndex::with_prefix("timestamping.payments", prefix, &self.view)
    }

    // TODO Wonder if the proofs is needed here?
    pub fn known_keys(&self) -> MapIndex<&T, PublicKey, Vec<u8>> {
        MapIndex::new("timestamping.known_keys", &self.view)
    }

    pub fn timestamps_history(&self, user_id: &str) -> ListIndex<&T, Hash> {
        let prefix = gen_prefix(&user_id.to_owned());
        ListIndex::with_prefix("timestamping.timestamps_history", prefix, &self.view)
    }

    pub fn users_history(&self) -> ListIndex<&T, Hash> {
        ListIndex::new("timestamping.users_history", &self.view)
    }

    pub fn state_hash(&self) -> Vec<Hash> {
        vec![self.timestamps().root_hash(), self.users().root_hash()]
    }
}

impl<'a> Schema<&'a mut Fork> {
    pub fn users_mut(&mut self) -> ProofMapIndex<&mut Fork, Hash, UserInfoEntry> {
        ProofMapIndex::new("timestamping.users", &mut self.view)
    }

    pub fn timestamps_mut(&mut self) -> ProofMapIndex<&mut Fork, Hash, TimestampEntry> {
        ProofMapIndex::new("timestamping.timestamps", &mut self.view)
    }

    pub fn payments_mut(&mut self, user_id: &str) -> ProofListIndex<&mut Fork, PaymentInfo> {
        let prefix = gen_prefix(&user_id.to_owned());
        ProofListIndex::with_prefix("timestamping.payments", prefix, &mut self.view)
    }

    pub fn known_keys_mut(&mut self) -> MapIndex<&mut Fork, PublicKey, Vec<u8>> {
        MapIndex::new("timestamping.known_keys", &mut self.view)
    }

    pub fn timestamps_history_mut(&mut self, user_id: &str) -> ListIndex<&mut Fork, Hash> {
        let prefix = gen_prefix(&user_id.to_owned());
        ListIndex::with_prefix("timestamping.timestamps_history", prefix, &mut self.view)
    }

    pub fn users_history_mut(&mut self) -> ListIndex<&mut Fork, Hash> {
        ListIndex::new("timestamping.users_history", &mut self.view)
    }

    pub fn add_user(&mut self, user_id_hash: Hash, user: UserInfo) {
        // Add user key to known.
        self.known_keys_mut().put(
            user.pub_key(),
            user.encrypted_secret_key().to_vec(),
        );
        // Add or modify user.
        let entry = if let Some(entry) = self.users().get(&user_id_hash) {
            // Modify existing user
            UserInfoEntry::new(user, entry.available_timestamps(), entry.payments_hash())
        } else {
            // Add user to history
            self.users_history_mut().push(user_id_hash);
            UserInfoEntry::new(user, INITIAL_TIMESTAMPS, &Hash::zero())
        };
        self.users_mut().put(&user_id_hash, entry);
    }

    pub fn add_payment(&mut self, payment: PaymentInfo) {
        let user_id = payment.user_id().to_owned();
        let user_id_hash = user_id.to_hash();
        if let Some(entry) = self.users().get(&user_id_hash) {
            // TODO check type safety
            let total_amount = payment.total_amount() as i64;
            self.payments_mut(&user_id).push(payment);
            // Update user info
            let entry = UserInfoEntry::new(
                entry.info(),
                total_amount,
                &self.payments(&user_id).root_hash(),
            );
            self.users_mut().put(&user_id_hash, entry);
        }
    }

    pub fn add_timestamp(&mut self, timestamp_entry: TimestampEntry) {
        let timestamp = timestamp_entry.timestamp();
        let content_hash = *timestamp.content_hash();
        // Check that timestamp with given content_hash does not exist.
        if self.timestamps().contains(&content_hash) {
            return;
        }

        let user_id = timestamp.user_id().to_owned();
        let user_id_hash = user_id.to_hash();
        if let Some(entry) = self.users().get(&user_id_hash) {
            // Add timestamp
            self.timestamps_mut().put(&content_hash, timestamp_entry);
            self.timestamps_history_mut(&user_id).push(content_hash);
            // Update user info
            let entry = UserInfoEntry::new(
                entry.info(),
                entry.available_timestamps() - 1,
                entry.payments_hash(),
            );
            self.users_mut().put(&user_id_hash, entry);
        }
    }
}
