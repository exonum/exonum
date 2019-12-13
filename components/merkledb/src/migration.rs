//! Migration utilities.

use exonum_crypto::Hash;

use std::{fmt, mem, sync::Arc};

use crate::{
    access::Prefixed, validation::assert_valid_name_component, Database, Fork, ReadonlyFork,
    SystemSchema,
};

/// Migration helper.
pub struct Migration {
    db: Arc<dyn Database>,
    fork: Fork,
    namespace: String,
}

impl fmt::Debug for Migration {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.debug_tuple("Migration").field(&self.namespace).finish()
    }
}

impl Migration {
    /// Creates a new helper.
    pub fn new(db: impl Into<Arc<dyn Database>>, namespace: &str) -> Self {
        assert_valid_name_component(namespace);

        let db = db.into();
        let fork = db.fork();
        Self {
            db,
            fork,
            namespace: namespace.to_owned(),
        }
    }

    /// Returns full access to the new version of migrated data.
    pub fn new_data(&self) -> Prefixed<'_, &Fork> {
        Prefixed::for_migration(&self.namespace, &self.fork)
    }

    /// Returns readonly access to the old version of migrated data.
    pub fn old_data(&self) -> Prefixed<'_, ReadonlyFork<'_>> {
        Prefixed::new(&self.namespace, self.fork.readonly())
    }

    /// Merges the changes to the migrated data to the database. Returns an error
    /// if the merge has failed.
    pub fn merge(&mut self) -> crate::Result<()> {
        let fork = mem::replace(&mut self.fork, self.db.fork());
        self.db.merge(fork.into_patch())?;
        self.fork = self.db.fork();
        Ok(())
    }

    /// Merges the changes to the migrated data to the database.
    /// Returns hash representing migrated data state, or an error if the merge has failed.
    pub fn finish(self) -> crate::Result<Hash> {
        let patch = self.fork.into_patch();
        let hash = SystemSchema::new(&patch).namespace_state_hash(&self.namespace);
        self.db.merge(patch).map(|()| hash)
    }
}
