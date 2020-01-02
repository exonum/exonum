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

//! Validation helpers for index names.

use crate::access::AccessErrorKind;

/// Validates that an index `name` consists of allowed chars. This method does not check
/// if `name` is empty.
pub fn is_valid_identifier(name: &str) -> bool {
    name.as_bytes()
        .iter()
        .all(|&c| is_allowed_index_name_char(c) || c == b'.')
}

/// Validates that a `prefix` consists of chars allowed for an index prefix.
///
/// Unlike [full names], prefixes are not allowed to contain a dot char `'.'`.
///
/// [full names]: fn.is_valid_identifier.html
pub fn is_valid_index_name_component(prefix: &str) -> bool {
    prefix
        .as_bytes()
        .iter()
        .copied()
        .all(is_allowed_index_name_char)
}

/// Checks that a character is allowed in an index name.
///
/// Only these combination of symbols are allowed:
/// `[0-9]`, `[a-z]`, `[A-Z]`, `_`, `-`.
pub fn is_allowed_index_name_char(c: u8) -> bool {
    match c {
        b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' | b'-' | b'_' => true,
        _ => false,
    }
}

// Allow because it's looks more readable.
#[allow(clippy::if_not_else)]
fn check_valid_name<F>(
    name: &str,
    predicate: F,
    allowed_chars: &'static str,
) -> Result<(), AccessErrorKind>
where
    F: Fn(&str) -> bool,
{
    if name.is_empty() {
        Err(AccessErrorKind::EmptyName)
    } else if !predicate(name) {
        Err(AccessErrorKind::InvalidCharsInName {
            name: name.to_owned(),
            allowed_chars,
        })
    } else {
        Ok(())
    }
}

/// Checks that provided address is valid index full name.
pub(crate) fn check_index_valid_full_name(name: &str) -> Result<(), AccessErrorKind> {
    if name.starts_with("__") && !name.contains('.') {
        return Err(AccessErrorKind::ReservedName);
    }
    check_valid_name(name, is_valid_identifier, "a-zA-Z0-9 and _-.")
}

pub(crate) fn assert_valid_name_component(name: &str) {
    check_valid_name(name, is_valid_index_name_component, "a-zA-Z0-9 and _-").unwrap();
}

#[cfg(test)]
mod test {
    use assert_matches::assert_matches;

    use crate::{
        access::{AccessErrorKind, FromAccess},
        Database, ListIndex, TemporaryDB,
    };

    #[test]
    fn index_name_validation() {
        let db = TemporaryDB::new();
        let fork = db.fork();

        let e = ListIndex::<_, u32>::from_access(&fork, "".into()).unwrap_err();
        assert_matches!(e.kind, AccessErrorKind::EmptyName);
        let e = ListIndex::<_, u32>::from_access(&fork, "__METADATA__".into()).unwrap_err();
        assert_matches!(e.kind, AccessErrorKind::ReservedName);
        let e = ListIndex::<_, u32>::from_access(&fork, "__system_index".into()).unwrap_err();
        assert_matches!(e.kind, AccessErrorKind::ReservedName);
        let res = ListIndex::<_, u32>::from_access(&fork, "__SYSTEM.INDEX__".into());
        assert!(res.is_ok());

        // spell-checker:disable
        let e = ListIndex::<_, u32>::from_access(
            &fork,
            "\u{441}\u{43f}\u{438}\u{441}\u{43e}\u{43a}".into(),
        )
        .unwrap_err();
        assert_matches!(e.kind, AccessErrorKind::InvalidCharsInName { .. });
    }
}
