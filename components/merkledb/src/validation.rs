// Copyright 2019 The Exonum Team
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

use crate::access::{AccessError, AccessErrorKind};
use crate::IndexAddress;

/// Validates index name.
pub fn is_valid_index_full_name(name: &str) -> bool {
    name.as_bytes()
        .iter()
        .all(|&c| is_allowed_index_name_char(c) || c == b'.')
}

/// Validates index name prefix, it shouldn't contain the dot.
pub fn is_valid_index_name_component(name: &str) -> bool {
    name.as_bytes()
        .iter()
        .copied()
        .all(is_allowed_index_name_char)
}

/// Checks that character is allowed in index name.
/// Only these combination of symbols are allowed:
///
/// `[0..9]`, `[a-z]`, `[A-Z]`, `_`, `-`
pub fn is_allowed_index_name_char(c: u8) -> bool {
    match c {
        b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' | b'-' | b'_' => true,
        _ => false,
    }
}

// Allow because it's looks more readable.
#[allow(clippy::if_not_else)]
fn check_valid_name<F>(
    addr: IndexAddress,
    predicate: F,
    allowed_chars: &'static str,
) -> Result<(), AccessError>
where
    F: Fn(&str) -> bool,
{
    let name = &addr.name;

    if name.is_empty() {
        Err(AccessError {
            kind: AccessErrorKind::EmptyName,
            addr,
        })
    } else if !predicate(name) {
        Err(AccessError {
            kind: AccessErrorKind::InvalidCharsInName {
                name: name.clone(),
                allowed_chars,
            },
            addr,
        })
    } else {
        Ok(())
    }
}

/// Checks that provided address is valid index full name.
pub(crate) fn check_index_valid_full_name(addr: &IndexAddress) -> Result<(), AccessError> {
    let addr = addr.clone();

    if addr.name.starts_with("__") && !addr.name.contains('.') {
        return Err(AccessError {
            addr,
            kind: AccessErrorKind::ReservedName,
        });
    };

    check_valid_name(addr, is_valid_index_full_name, "a-zA-Z0-9 and _-.")
}

/// Calls the `is_valid_index_name_component` function with the given
/// name and panics if it returns `false`.
pub(crate) fn assert_valid_name_component(name: &str) {
    if let Err(access_error) = check_valid_name(
        name.into(),
        is_valid_index_name_component,
        "a-zA-Z0-9 and _-",
    ) {
        panic!(access_error.to_string())
    }
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
        let e = ListIndex::<_, u32>::from_access(&fork, "__SYSTEM.INDEX__".into());
        assert!(e.is_ok());

        // spell-checker:disable
        let e = ListIndex::<_, u32>::from_access(
            &fork,
            "\u{441}\u{43f}\u{438}\u{441}\u{43e}\u{43a}".into(),
        )
        .unwrap_err();
        assert_matches!(e.kind, AccessErrorKind::InvalidCharsInName { .. });
    }
}
