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

// Allow because it's looks more readable.
#[allow(clippy::if_not_else)]
fn check_valid_name<S: AsRef<str> + Copy, F>(
    name: S,
    predicate: F,
    desc: &str,
) -> Result<(), String>
where
    F: Fn(S) -> bool,
{
    if name.as_ref().is_empty() {
        Err("Index name must not be empty".into())
    } else if !predicate(name) {
        Err(format!(
            "Wrong characters using in name ({}). {}",
            name.as_ref(),
            desc
        ))
    } else {
        Ok(())
    }
}

/// Calls the `is_valid_index_full_name` function with the given index address.
pub(crate) fn check_index_valid_full_name(addr: &IndexAddress) -> Result<(), AccessError> {
    let name = &addr.name;
    let is_system = name.starts_with("__");

    match check_valid_name(name, is_valid_index_full_name, "Use: a-zA-Z0-9 and _-.") {
        Err(msg) => {
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
        _ => Ok(()),
    }
}

/// Calls the `is_valid_index_name_component` function with the given
/// name and panics if it returns `false`.
pub(crate) fn assert_valid_name_component(name: &str) {
    if let Err(msg) = check_valid_name(name, is_valid_index_name_component, "Use: a-zA-Z0-9 and _-")
    {
        panic!(msg)
    }
}
