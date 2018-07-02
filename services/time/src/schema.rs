use chrono::{DateTime, Utc};
use exonum::{
    crypto::{Hash, PublicKey}, storage::{Entry, Fork, ProofMapIndex, Snapshot},
};

/// `Exonum-time` service database schema.
#[derive(Debug)]
pub struct TimeSchema<T> {
    view: T,
}

impl<T: AsRef<Snapshot>> TimeSchema<T> {
    /// Constructs schema for the given `snapshot`.
    pub fn new(view: T) -> Self {
        TimeSchema { view }
    }

    /// Returns the table that stores `DateTime` for every validator.
    pub fn validators_times(&self) -> ProofMapIndex<&Snapshot, PublicKey, DateTime<Utc>> {
        ProofMapIndex::new("exonum_time.validators_times", self.view.as_ref())
    }

    /// Returns stored time.
    pub fn time(&self) -> Entry<&Snapshot, DateTime<Utc>> {
        Entry::new("exonum_time.time", self.view.as_ref())
    }

    /// Returns hashes for stored tables.
    pub fn state_hash(&self) -> Vec<Hash> {
        vec![self.validators_times().merkle_root(), self.time().hash()]
    }
}

impl<'a> TimeSchema<&'a mut Fork> {
    /// Mutable reference to the ['validators_times'][1] index.
    ///
    /// [1]: struct.TimeSchema.html#method.validators_times
    pub fn validators_times_mut(&mut self) -> ProofMapIndex<&mut Fork, PublicKey, DateTime<Utc>> {
        ProofMapIndex::new("exonum_time.validators_times", self.view)
    }

    /// Mutable reference to the ['time'][1] index.
    ///
    /// [1]: struct.TimeSchema.html#method.time
    pub fn time_mut(&mut self) -> Entry<&mut Fork, DateTime<Utc>> {
        Entry::new("exonum_time.time", self.view)
    }
}
