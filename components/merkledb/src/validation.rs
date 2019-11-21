//! Validation helpers for index names.

use core::fmt;

/// Validate index name.
pub fn is_valid_index_full_name<S: AsRef<str>>(name: S) -> bool {
    name.as_ref()
        .as_bytes()
        .iter()
        .copied()
        .all(|c| is_allowed_index_name_char(c) || c == 46)
}

/// Validate index name prefix, it shouldn't contain the dot.
pub fn is_valid_index_name_component<S: AsRef<str>>(name: S) -> bool {
    name.as_ref()
        .as_bytes()
        .iter()
        .copied()
        .all(is_allowed_index_name_char)
}

/// Check that character is allowed in index name.
/// Only these combination of symbols are allowed:
///
/// `[0..9]`, `[a-z]`, `[A-Z]`, `_`, `-`
pub fn is_allowed_index_name_char(c: u8) -> bool {
    match c {
        b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' | b'-' | b'_' => true,
        _ => false,
    }
}

fn assert_valid_name<S: AsRef<str> + Copy + fmt::Debug, F>(name: S, predicate: F, desc: &str)
where
    F: Fn(S) -> bool,
{
    if name.as_ref().is_empty() {
        panic!("Index name must not be empty")
    }

    if !predicate(name) {
        panic!("Wrong characters using in name ({:?}). {}", name, desc);
    }
}

/// Calls the `is_valid_name` function with the given name and panics if it returns `false`.
pub(crate) fn assert_index_valid_full_name<S: AsRef<str> + Copy + fmt::Debug>(name: S) {
    assert_valid_name(name, is_valid_index_full_name, "Use: a-zA-Z0-9 and _-.")
}

/// Calls the `is_valid_prefix` function with the given name and panics if it returns `false`.
pub(crate) fn assert_valid_index_name_component<S: AsRef<str> + Copy + fmt::Debug>(name: S) {
    assert_valid_name(name, is_valid_index_name_component, "Use: a-zA-Z0-9 and _-")
}
