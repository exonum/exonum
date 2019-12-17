use std::{borrow::Cow, num::NonZeroU64};

use crate::BinaryKey;

pub fn key_bytes<K: BinaryKey + ?Sized>(key: &K) -> Vec<u8> {
    concat_keys!(key)
}

/// Represents the address of an index in the database.
///
/// An address has a string *name* and an optional byte *key*. An index is uniquely identified
/// by its address. Different addresses correspond to different indexes. Addresses with the same
/// name and differing keys are said to belong to the same *group* (see also [`Group`]). Groups
/// can be used for a potentially unbounded group of indexes that can be identified by a certain
/// key (for example, `ProofListIndex` with the transaction history of a wallet keyed by the
/// `PublicKey` of the wallet).
///
/// [`Group`]: struct.Group.html
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
/// let addr = IndexAddress::from_root("data").append_key(&vec![1, 2, 3]);
/// let set = fork.get_value_set::<_, u64>(addr);
/// ```
#[derive(Debug, Clone, Eq, PartialEq, Hash, Default)]
pub struct IndexAddress {
    pub(super) name: String,
    pub(super) id_in_group: Option<Vec<u8>>,
}

impl IndexAddress {
    /// Creates new `IndexAddress` with the specified name.
    pub fn from_root<S: Into<String>>(root: S) -> Self {
        Self {
            name: root.into(),
            id_in_group: None,
        }
    }

    /// Returns the name part of `IndexAddress`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns a resolved address name, which can be used in `ResolvedAddress`. The resolved
    /// name corresponds to a usual `name()`, unless it starts with the "next version" char `^`,
    /// in which case the resolved name does not include this char.
    pub(super) fn resolved_name(&self) -> &str {
        if self.name.starts_with('^') {
            &self.name[1..]
        } else {
            &self.name
        }
    }

    /// Returns the namespace that the address belongs to. For migrated indexes, the namespace
    /// is defined as the component of the address name up (but not including) the first dot `'.'`
    /// char in the name (e.g., `foo` for address `^foo.bar`). For non-migrated indexes,
    /// the namespace is the empty string.
    pub(super) fn namespace(&self) -> &str {
        if self.name.starts_with('^') {
            let dot_position = self.name.find('.');
            if let Some(pos) = dot_position {
                &self.name[1..pos]
            } else {
                &self.name[1..]
            }
        } else {
            ""
        }
    }

    /// Returns the bytes part of `IndexAddress`.
    pub fn id_in_group(&self) -> Option<&[u8]> {
        self.id_in_group.as_ref().map(Vec::as_slice)
    }

    /// Prepends a name part to `IndexAddress`. The name is separated from the existing name
    /// by a dot `.`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use exonum_merkledb::IndexAddress;
    /// let addr = IndexAddress::from_root("foo");
    /// let prefixed = addr.prepend_name("prefix");
    /// assert_eq!(prefixed.name(), "prefix.foo");
    /// ```
    pub fn prepend_name(self, prefix: &str) -> Self {
        let name = if self.name.is_empty() {
            prefix.to_owned()
        } else {
            // Because `concat` is faster than `format!("...")` in all cases.
            [prefix, ".", self.name()].concat()
        };
        Self {
            name,
            id_in_group: self.id_in_group,
        }
    }

    /// Appends a name part to `IndexAddress`. The name is separated from the existing name
    /// by a dot `.`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use exonum_merkledb::IndexAddress;
    /// let addr = IndexAddress::from_root("foo");
    /// let suffixed = addr.append_name("suffix");
    /// assert_eq!(suffixed.name(), "foo.suffix");
    /// ```
    pub fn append_name(self, suffix: &str) -> Self {
        let name = if self.name.is_empty() {
            suffix.to_owned()
        } else {
            // Because `concat` is faster than `format!("...")` in all cases.
            [self.name(), ".", suffix].concat()
        };
        Self {
            name,
            id_in_group: self.id_in_group,
        }
    }

    /// Appends a key to the `IndexAddress`.
    pub fn append_key<K: BinaryKey + ?Sized>(self, suffix: &K) -> Self {
        let name = self.name;
        let bytes = if let Some(ref bytes) = self.id_in_group {
            concat_keys!(bytes, suffix)
        } else {
            concat_keys!(suffix)
        };

        Self {
            name,
            id_in_group: Some(bytes),
        }
    }

    /// Full address with a separator between `name` and `bytes` represented as byte array.
    pub(crate) fn fully_qualified_name(&self) -> Vec<u8> {
        /// Separator between the name and the additional bytes in family indexes.
        const INDEX_NAME_SEPARATOR: &[u8] = &[0];

        if let Some(bytes) = self.id_in_group() {
            concat_keys!(self.name(), INDEX_NAME_SEPARATOR, bytes)
        } else {
            concat_keys!(self.name())
        }
    }
}

impl<'a> From<&'a str> for IndexAddress {
    fn from(name: &'a str) -> Self {
        Self::from_root(name)
    }
}

impl From<String> for IndexAddress {
    fn from(name: String) -> Self {
        Self::from_root(name)
    }
}

impl<'a, K: BinaryKey + ?Sized> From<(&'a str, &'a K)> for IndexAddress {
    fn from((name, key): (&'a str, &'a K)) -> Self {
        Self {
            name: name.to_owned(),
            id_in_group: Some(key_bytes(key)),
        }
    }
}

/// Resolved address of a view.
///
/// While an `IndexAddress` is a logical location of a view, a `ResolvedAddress`
/// represents its location in the key-value storage. Mapping between `IndexAddress`es
/// and `ResolvedAddress`es is internal to the database logic.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ResolvedAddress {
    /// Name of the column family where the view is stored.
    pub name: String,
    /// Index identifier within a column family.
    pub id: Option<NonZeroU64>,
}

impl ResolvedAddress {
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

// This conversion is only useful for tests, since all user-created indexes should have an ID set.
#[cfg(test)]
impl From<&str> for ResolvedAddress {
    fn from(name: &str) -> Self {
        Self::system(name)
    }
}
