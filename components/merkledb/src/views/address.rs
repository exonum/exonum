// Copyright 2020 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::{borrow::Cow, num::NonZeroU64};

use crate::BinaryKey;

pub fn key_bytes<K: BinaryKey + ?Sized>(key: &K) -> Vec<u8> {
    concat_keys!(key)
}

const SEPARATOR_CHAR: u8 = 0;
const MIGRATION_CHAR: u8 = b'^';

/// Represents the address of an index in the database.
///
/// An address has a string *name* and an optional byte *key*. An index is uniquely identified
/// by its address. Different addresses correspond to different indexes. Addresses with the same
/// name and differing keys are said to belong to the same *group* (see also [`Group`]). Groups
/// can be used for a potentially unbounded group of indexes that can be identified by a certain
/// key (for example, `ProofListIndex` with the transaction history of a wallet keyed by the
/// `PublicKey` of the wallet).
///
/// In contrast with [`ResolvedAddress`], `IndexAddress` is a high-level logical construct;
/// it does not directly map to key-value storage abstractions (column families and their keys).
///
/// [`Group`]: indexes/group/struct.Group.html
/// [`ResolvedAddress`]: struct.ResolvedAddress.html
///
/// # Examples
///
/// `IndexAddress` can be used implicitly, since `&str` and `(&str, &impl BinaryKey)` can both
/// be converted into an address.
///
/// ```
/// use exonum_merkledb::{access::CopyAccessExt, IndexAddress, TemporaryDB, Database};
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
    pub(super) in_migration: bool,
}

impl IndexAddress {
    /// Creates new `IndexAddress` with the specified name.
    pub fn from_root<S: Into<String>>(root: S) -> Self {
        Self {
            name: root.into(),
            id_in_group: None,
            in_migration: false,
        }
    }

    /// Returns the name part of `IndexAddress`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the namespace that the address belongs to. For migrated indexes, the namespace
    /// is defined as the component of the address name up (but not including) the first dot `'.'`
    /// char in the name (e.g., `foo` for address `^foo.bar`). For non-migrated indexes,
    /// the namespace is the empty string.
    pub(super) fn namespace(&self) -> &str {
        if self.in_migration {
            let dot_position = self.name.find('.');
            if let Some(pos) = dot_position {
                &self.name[..pos]
            } else {
                &self.name
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

        Self { name, ..self }
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

        Self { name, ..self }
    }

    /// Appends a key to the `IndexAddress`.
    pub fn append_key<K: BinaryKey + ?Sized>(self, suffix: &K) -> Self {
        let bytes = if let Some(ref bytes) = self.id_in_group {
            concat_keys!(bytes, suffix)
        } else {
            concat_keys!(suffix)
        };

        Self {
            id_in_group: Some(bytes),
            ..self
        }
    }

    pub(crate) fn set_in_migration(&mut self) {
        self.in_migration = true;
    }

    /// Full address with a separator between `name` and `bytes` represented as byte array.
    pub(crate) fn fully_qualified_name(&self) -> Vec<u8> {
        /// Separator between the name and the additional bytes in family indexes.
        const INDEX_NAME_SEPARATOR: &[u8] = &[SEPARATOR_CHAR];
        const MIGRATION_PREFIX: &[u8] = &[MIGRATION_CHAR];

        match (self.in_migration, self.id_in_group()) {
            (true, Some(bytes)) => {
                concat_keys!(MIGRATION_PREFIX, self.name(), INDEX_NAME_SEPARATOR, bytes)
            }
            (false, Some(bytes)) => concat_keys!(self.name(), INDEX_NAME_SEPARATOR, bytes),
            (true, None) => concat_keys!(MIGRATION_PREFIX, self.name()),
            (false, None) => self.name.as_bytes().to_vec(),
        }
    }

    /// Returns the common prefix of fully qualified names for the child indexes.
    pub(crate) fn qualified_prefix(&self) -> Vec<u8> {
        let mut prefix = self.fully_qualified_name();
        if self.id_in_group.is_none() {
            prefix.push(SEPARATOR_CHAR);
        }
        prefix
    }

    /// Infers the name part of the fully qualified name that was obtained with
    /// `fully_qualified_name`. This is the part corresponding to `ResolvedAddress.name`.
    /// `min_name_len` specifies the minimum known length of the name part.
    ///
    /// # Return value
    ///
    /// Returns the restored name part of the index address and a bool flag indicating whether
    /// the index is a part of a group.
    #[allow(unsafe_code)]
    pub(super) fn parse_fully_qualified_name(
        qualified_name: &[u8],
        min_name_len: usize,
    ) -> (String, bool) {
        let (cutoff_index, is_in_group) = qualified_name[min_name_len..]
            .iter()
            .position(|&byte| byte == SEPARATOR_CHAR)
            .map_or_else(
                || (qualified_name.len(), false),
                |pos| (pos + min_name_len, true),
            );

        let name = if qualified_name[0] == MIGRATION_CHAR {
            // The `^` initial char is not mapped to the resolved CF name.
            qualified_name[1..cutoff_index].to_vec()
        } else {
            qualified_name[..cutoff_index].to_vec()
        };
        debug_assert!(
            name.is_ascii(),
            "BUG: non-ASCII chars in the name part of a fully qualified index name \
             (qualified name = {:?}, inferred name part = {})",
            qualified_name,
            String::from_utf8_lossy(&name)
        );

        let name = unsafe {
            // SAFETY:
            // Safe by construction; metadata keys before the `\0` separator
            // correspond to the index names, which consist of a subset of
            // ASCII chars.
            String::from_utf8_unchecked(name)
        };
        (name, is_in_group)
    }

    /// Converts a migration namespace into the form that all indexes in the namespace
    /// begin with.
    #[inline]
    pub(super) fn qualify_migration_namespace(namespace: &str) -> Vec<u8> {
        ["^", namespace, "."].concat().into_bytes()
    }

    #[inline]
    pub(super) fn migrate_qualified_name(qualified_name: &[u8]) -> &[u8] {
        debug_assert_eq!(
            qualified_name[0], MIGRATION_CHAR,
            "Qualified name {:?} is not in migration",
            qualified_name
        );
        &qualified_name[1..]
    }
}

impl From<&str> for IndexAddress {
    fn from(name: &str) -> Self {
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
            in_migration: false,
        }
    }
}

/// Resolved address of a view.
///
/// While an [`IndexAddress`] is a logical location of a view, a `ResolvedAddress`
/// represents its location in the key-value storage. The mapping between `IndexAddress`es
/// and `ResolvedAddress`es is internal to the database logic.
///
/// [`IndexAddress`]: struct.IndexAddress.html
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ResolvedAddress {
    /// Name of the column family where the view is stored.
    pub name: String,
    /// Index identifier within a column family. If set to `None`, then the column family contains
    /// a single view. If `id` is `Some(_)`, then its value should be used to split key spaces
    /// for different views in the same column family. In other words, key spaces for two addresses
    /// with equal `name` and `id`s `Some(x)` and `Some(y)`, `x != y`, must not intersect.
    pub id: Option<NonZeroU64>,
}

impl ResolvedAddress {
    pub(crate) fn new(name: impl Into<String>, id: Option<NonZeroU64>) -> Self {
        Self {
            name: name.into(),
            id,
        }
    }

    /// Creates a system view. System views are low-level (i.e., they are not wrapped in indexes).
    pub(crate) fn system(name: impl Into<String>) -> Self {
        Self::new(name, None)
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

#[test]
fn address_resolution() {
    {
        let (name, is_in_group) = IndexAddress::parse_fully_qualified_name(b"some.list", 0);
        assert_eq!(name, "some.list");
        assert!(!is_in_group);
    }
    {
        let (name, is_in_group) = IndexAddress::parse_fully_qualified_name(b"some.list", 9);
        assert_eq!(name, "some.list");
        assert!(!is_in_group);
    }
    {
        let (name, is_in_group) =
            IndexAddress::parse_fully_qualified_name(b"some.list\0key\xa0", 9);
        assert_eq!(name, "some.list");
        assert!(is_in_group);
    }
}
