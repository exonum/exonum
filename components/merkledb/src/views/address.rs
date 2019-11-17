use std::{borrow::Cow, num::NonZeroU64};

use crate::BinaryKey;

pub fn key_bytes<K: BinaryKey + ?Sized>(key: &K) -> Vec<u8> {
    concat_keys!(key)
}

/// Represents address of the index in the database.
///
/// # Examples
///
/// `IndexAddress` can be used implicitly, since `&str` and `(&str, &impl BinaryKey)` can both
/// be converted into an address.
///
/// ```
/// use exonum_merkledb::{access::AccessExt, IndexAddress, TemporaryDB, Database};
///
/// let db = TemporaryDB::new();
/// let fork = db.fork();
///
/// // Using a string address:
/// let map = fork.get_map::<_, String, u8>("map");
/// // Using an address within an index family:
/// let list = fork.get_list::<_, String>(("index", &3_u32));
/// // Using `IndexAddress` explicitly:
/// let addr = IndexAddress::with_root("data").append_bytes(&vec![1, 2, 3]);
/// let set = fork.get_value_set::<_, u64>(addr);
/// ```
#[derive(Debug, Clone, Eq, PartialEq, Hash, Default)]
pub struct IndexAddress {
    pub(super) name: String,
    pub(super) bytes: Option<Vec<u8>>,
}

impl IndexAddress {
    /// Creates empty `IndexAddress`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates new `IndexAddress` with specified `root` name.
    pub fn with_root<S: Into<String>>(root: S) -> Self {
        Self {
            name: root.into(),
            bytes: None,
        }
    }

    /// Returns name part of `IndexAddress`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns bytes part of `IndexAddress`.
    pub fn bytes(&self) -> Option<&[u8]> {
        self.bytes.as_ref().map(Vec::as_slice)
    }

    /// Prepends a name part to `IndexAddress`. The name is separated from the existing name
    /// by a dot `.`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use exonum_merkledb::IndexAddress;
    /// let addr = IndexAddress::with_root("foo");
    /// let prefixed = addr.prepend_name("prefix");
    /// assert_eq!(prefixed.name(), "prefix.foo");
    /// ```
    pub fn prepend_name<'a>(self, prefix: impl Into<Cow<'a, str>>) -> Self {
        let prefix = prefix.into();
        Self {
            name: if self.name.is_empty() {
                prefix.into_owned()
            } else {
                // Because `concat` is faster than `format!("...")` in all cases.
                [prefix.as_ref(), ".", self.name()].concat()
            },

            bytes: self.bytes,
        }
    }

    /// Appends a name part to `IndexAddress`. The name is separated from the existing name
    /// by a dot `.`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use exonum_merkledb::IndexAddress;
    /// let addr = IndexAddress::with_root("foo");
    /// let suffixed = addr.append_name("suffix");
    /// assert_eq!(suffixed.name(), "foo.suffix");
    /// ```
    pub fn append_name<'a>(self, suffix: impl Into<Cow<'a, str>>) -> Self {
        let suffix = suffix.into();
        Self {
            name: if self.name.is_empty() {
                suffix.into_owned()
            } else {
                // Because `concat` is faster than `format!("...")` in all cases.
                [self.name(), ".", suffix.as_ref()].concat()
            },

            bytes: self.bytes,
        }
    }

    /// Appends a bytes part to `IndexAddress`.
    pub fn append_bytes<K: BinaryKey + ?Sized>(self, suffix: &K) -> Self {
        let name = self.name;
        let bytes = if let Some(ref bytes) = self.bytes {
            concat_keys!(bytes, suffix)
        } else {
            concat_keys!(suffix)
        };

        Self {
            name,
            bytes: Some(bytes),
        }
    }

    /// Full address with a separator between `name` and `bytes` represented as byte array.
    pub fn fully_qualified_name(&self) -> Vec<u8> {
        /// Separator between the name and the additional bytes in family indexes.
        const INDEX_NAME_SEPARATOR: &[u8] = &[0];

        if let Some(bytes) = self.bytes() {
            concat_keys!(self.name(), INDEX_NAME_SEPARATOR, bytes)
        } else {
            concat_keys!(self.name())
        }
    }
}

impl<'a> From<&'a str> for IndexAddress {
    fn from(name: &'a str) -> Self {
        Self::with_root(name)
    }
}

impl From<String> for IndexAddress {
    fn from(name: String) -> Self {
        Self::with_root(name)
    }
}

impl<'a, K: BinaryKey + ?Sized> From<(&'a str, &'a K)> for IndexAddress {
    fn from((name, key): (&'a str, &'a K)) -> Self {
        Self {
            name: name.to_owned(),
            bytes: Some(key_bytes(key)),
        }
    }
}

/// Reference to a resolved address of a view.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ResolvedRef {
    /// Name of the column family where the view is stored.
    pub name: String,
    /// Index identifier within a column family.
    pub id: Option<NonZeroU64>,
}

#[cfg(test)] // All user-created indexes should have an ID set.
impl From<&str> for ResolvedRef {
    fn from(name: &str) -> Self {
        Self::system(name)
    }
}

#[cfg(test)]
impl From<(&str, u64)> for ResolvedRef {
    fn from((name, id): (&str, u64)) -> Self {
        Self {
            name: name.to_owned(),
            id: NonZeroU64::new(id),
        }
    }
}

impl ResolvedRef {
    /// Creates a system view. System views are low-level (i.e., they are not wrapped in indexes).
    pub(crate) fn system(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            id: None,
        }
    }

    pub(crate) fn id_to_bytes(&self) -> Option<[u8; 8]> {
        self.id.map(|id| id.get().to_le_bytes())
    }

    /// Returns `key` prefixed by the `id`.
    pub(crate) fn keyed<'k>(&self, key: &'k [u8]) -> Cow<'k, [u8]> {
        match self.id {
            None => Cow::Borrowed(key),
            Some(id) => {
                let mut bytes = Vec::with_capacity(8 + key.len());
                bytes.extend_from_slice(&id.get().to_le_bytes());
                bytes.extend_from_slice(key);
                bytes.into()
            }
        }
    }
}
