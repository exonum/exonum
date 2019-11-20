//! Validation helpers for index names.

use core::fmt;

/// Validate index name.
pub fn is_valid_index_name<S: AsRef<str>>(name: S) -> bool {
    name.as_ref()
        .as_bytes()
        .iter()
        .copied()
        .all(|c| is_allowed_latin1_char(c) || c == 46)
}

pub fn is_valid_service_name<S: AsRef<str>>(name: S) -> bool {
    name.as_ref()
        .as_bytes()
        .iter()
        .copied()
        .all(is_allowed_latin1_char)
}

/// Validate index name.
pub fn is_valid_artifact_name<S: AsRef<[u8]>>(name: S) -> bool {
    name.as_ref()
        .iter()
        .copied()
        .all(|c| is_allowed_latin1_char(c) || c == 46 || c == 58)
}

/// Check that latin1 character is allowed in index name.
/// Only these combination of symbols are allowed:
///
/// `[0..9]`, `[a-z]`, `[A-Z]`, `_`, `-`, `.`
fn is_allowed_latin1_char(c: u8) -> bool {
    match c {
        48..=57     // 0..9
        | 65..=90   // A..Z
        | 97..=122  // a..z
        | 45        // -
        | 95        // _
        => true,
        _ => false,
    }
}

/// Calls the `is_valid_name` function with the given name and panics if it returns `false`.
pub(crate) fn assert_index_valid_name<S: AsRef<str> + Clone + fmt::Debug>(name: S) {
    dbg!(name.as_ref());
    if name.as_ref().is_empty() {
        panic!("Index name must not be empty")
    }

    if !is_valid_index_name(name.clone()) {
        panic!("Wrong characters using in name({:?}). Use: a-zA-Z0-9 and _-", name);
    }
}

/// Calls the `is_valid_name` function with the given name and panics if it returns `false`.
pub(crate) fn assert_valid_prefix_name<S: AsRef<str> + Clone + fmt::Debug>(name: S) {
    dbg!(name.as_ref());
    if name.as_ref().is_empty() {
        panic!("Index name must not be empty")
    }

    if !is_valid_service_name(name.clone()) {
        panic!("Wrong characters using in name({:?}). Use: a-zA-Z0-9 and _-", name);
    }
}

pub fn check_valid_service_name(name: impl AsRef<str>) -> Result<(), failure::Error> {
    dbg!(name.as_ref());
    let name = name.as_ref();
    ensure!(
            !name.is_empty(),
            "Service instance name should not be empty"
        );
    ensure!(
            is_valid_service_name(name),
            "Service instance name contains illegal character, use only: a-zA-Z0-9 and one of _-."
        );
    dbg!("ok");
    Ok(())
}

pub fn check_valid_artifact_name(name: impl AsRef<str>) -> Result<(), failure::Error> {
    dbg!(name.as_ref());
    let name = name.as_ref();
    ensure!(
            !name.is_empty(),
            "Service instance name should not be empty"
        );
    ensure!(
            is_valid_artifact_name(name),
        );
    dbg!("ok");
    Ok(())
}
