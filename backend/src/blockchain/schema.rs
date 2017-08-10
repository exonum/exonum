use exonum::crypto::{Hash, PublicKey};
use exonum::storage::{MapIndex, ProofMapIndex, ProofListIndex, Snapshot,
                      Fork};
use exonum::blockchain::gen_prefix;
use exonum::messages::Message;

use TIMESTAMPING_SERVICE;
use blockchain::ToHash;
use blockchain::dto::{UserInfoEntry, UserInfo, Timestamp, TimestampEntry, PaymentInfo};

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
        let prefix = gen_prefix(TIMESTAMPING_SERVICE, 0, &());
        ProofMapIndex::new(prefix, &self.view)
    }

    pub fn timestamps(&self, user_id: &str) -> ProofMapIndex<&T, Hash, TimestampEntry> {
        let prefix = gen_prefix(TIMESTAMPING_SERVICE, 1, &user_id.to_owned());
        ProofMapIndex::new(prefix, &self.view)
    }

    pub fn payments(&self, user_id: &str) -> ProofListIndex<&T, PaymentInfo> {
        let prefix = gen_prefix(TIMESTAMPING_SERVICE, 2, &user_id.to_owned());
        ProofListIndex::new(prefix, &self.view)
    }

    pub fn known_keys(&self) -> MapIndex<&T, PublicKey, String> {
        let prefix = gen_prefix(TIMESTAMPING_SERVICE, 3, &());
        MapIndex::new(prefix, &self.view)
    }

    pub fn state_hash(&self) -> Vec<Hash> {
        vec![self.users().root_hash()]
    }
}

impl<'a> Schema<&'a mut Fork> {
    pub fn users_mut(&mut self) -> ProofMapIndex<&mut Fork, Hash, UserInfoEntry> {
        let prefix = gen_prefix(TIMESTAMPING_SERVICE, 0, &());
        ProofMapIndex::new(prefix, &mut self.view)
    }

    pub fn timestamps_mut(&mut self, user_id: &str) -> ProofMapIndex<&mut Fork, Hash, TimestampEntry> {
        let prefix = gen_prefix(TIMESTAMPING_SERVICE, 1, &user_id.to_owned());
        ProofMapIndex::new(prefix, &mut self.view)
    }

    pub fn payments_mut(&mut self, user_id: &str) -> ProofListIndex<&mut Fork, PaymentInfo> {
        let prefix = gen_prefix(TIMESTAMPING_SERVICE, 2, &user_id.to_owned());
        ProofListIndex::new(prefix, &mut self.view)
    }

    pub fn known_keys_mut(&mut self) -> MapIndex<&mut Fork, PublicKey, String> {
        let prefix = gen_prefix(TIMESTAMPING_SERVICE, 3, &());
        MapIndex::new(prefix, &mut self.view)
    }

    pub fn add_user(&mut self, user: UserInfo) {
        let user_id = user.id().to_hash();
        let entry = if let Some(entry) = self.users().get(&user_id) {
            self.known_keys_mut().put(
                user.pub_key(),
                user.encrypted_secret_key().into(),
            );
            UserInfoEntry::new(
                user,
                entry.available_timestamps(),
                entry.timestamps_hash(),
                entry.payments_hash(),
            )
        } else {
            UserInfoEntry::new(user, INITIAL_TIMESTAMPS, &Hash::zero(), &Hash::zero())
        };
        self.users_mut().put(&user_id, entry);
    }

    pub fn add_payment(&mut self, payment: PaymentInfo) {
        let user_id = payment.user_id().to_owned();
        let user_id_hash = user_id.to_hash();
        if let Some(entry) = self.users().get(&user_id_hash) {
            self.payments_mut(&user_id).push(payment);
            // Update user info
            let entry = UserInfoEntry::new(
                entry.info(),
                entry.available_timestamps(),
                entry.timestamps_hash(),
                &self.payments(&user_id).root_hash(),
            );
            self.users_mut().put(&user_id_hash, entry);
        }
    }

    pub fn add_timestamp(&mut self, timestamp_entry: TimestampEntry) {
        let timestamp = timestamp_entry.timestamp();
        let user_id = timestamp.user_id().to_owned();
        let user_id_hash = user_id.to_hash();
        if let Some(entry) = self.users().get(&user_id_hash) {
            let content_hash = *timestamp.content_hash();
            self.timestamps_mut(&user_id).put(&content_hash, timestamp_entry);
            // Update user info
            let entry = UserInfoEntry::new(
                entry.info(),
                entry.available_timestamps() - 1,
                &self.timestamps(&user_id).root_hash(),
                entry.payments_hash(),
            );
            self.users_mut().put(&user_id_hash, entry);
        }
    }
}