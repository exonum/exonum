use std::collections::BTreeMap;

use super::{Map, Result};

// TODO In this implementation there are extra memory allocations when key is passed into specific database.
// Think about key type. Maybe we can use keys with fixed length?
pub trait Database: Sized + Clone + Send + Sync + 'static {
    type Fork: Fork;

    // fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>>;
    // fn put(&self, key: &[u8], value: Vec<u8>) -> Result<()>;
    // fn delete(&self, key: &[u8]) -> Result<()>;

    fn fork(&self) -> Self::Fork;
    fn merge(&self, patch: &Patch) -> Result<()>;
}

pub trait Fork: Map<[u8], Vec<u8>> + Sized {
    fn changes(&self) -> Patch;
    fn merge(&self, patch: &Patch);
}

#[derive(Clone)]
pub enum Change {
    Put(Vec<u8>),
    Delete,
}

pub type Patch = BTreeMap<Vec<u8>, Change>;
