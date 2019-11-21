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
pub fn is_valid_index_full_name<S: AsRef<str>>(name: S) -> bool {
    name.as_ref()
        .as_bytes()
        .iter()
        .all(|c| is_allowed_index_name_char(*c) || *c == b'.')
}

/// Validates index name prefix, it shouldn't contain the dot.
pub fn is_valid_index_name_component<S: AsRef<str>>(name: S) -> bool {
    name.as_ref()
        .as_bytes()
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

fn assert_valid_name<S: AsRef<str> + Copy, F>(name: S, predicate: F, desc: &str)
where
    F: Fn(S) -> bool,
{
    if name.as_ref().is_empty() {
        panic!("Index name must not be empty")
    }

    if !predicate(name) {
        panic!(
            "Wrong characters using in name ({}). {}",
            name.as_ref(),
            desc
        );
    }
}

/// Calls the `is_valid_index_full_name` function with the given name
/// and panics if it returns `false`.
pub(crate) fn assert_index_valid_full_name(addr: &IndexAddress) -> Result<(), AccessError> {
    let name = addr.name.clone();
    let is_system = name.starts_with("__");

    let msg = if name.is_empty() {
        "Index name must not be empty".into()
    } else if !is_valid_index_full_name(&name) {
        format!("Wrong characters using in name ({}). Use: a-zA-Z0-9 and _-.", name)
    } else {
        return Ok(())
    };

    let kind = if is_system {
        AccessErrorKind::InvalidIndexName(msg)
    } else {
        AccessErrorKind::InvalidSystemIndexName(msg)
    };

    Err(AccessError {
        addr: addr.clone(),
        kind,
    })
}

/// Calls the `is_valid_index_name_component` function with the given
/// name and panics if it returns `false`.
pub(crate) fn assert_valid_name_component(name: &str) {
    assert_valid_name(name, is_valid_index_name_component, "Use: a-zA-Z0-9 and _-")
}
