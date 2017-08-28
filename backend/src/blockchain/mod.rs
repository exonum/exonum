pub mod dto;
pub mod schema;
mod transactions;
#[cfg(test)]
mod tests;

use exonum::crypto::{hash, Hash};

pub(crate) trait ToHash {
    fn to_hash(&self) -> Hash;
}

impl<T> ToHash for T
where
    T: AsRef<str>,
{
    fn to_hash(&self) -> Hash {
        hash(self.as_ref().as_bytes())
    }
}
