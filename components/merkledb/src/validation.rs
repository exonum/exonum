//! Validation helpers for index names.

/// Validate index name.
pub fn is_valid_index_name<S: AsRef<str>>(name: S) -> bool {
    name.as_ref()
        .as_bytes()
        .iter()
        .copied()
        .all(is_allowed_latin1_char)
}

/// Check that latin1 character is allowed in index name.
/// Only these combination of symbols are allowed:
///
/// `[0..9]`, `[a-z]`, `[A-Z]`, `_`, `-`, `.`
pub fn is_allowed_latin1_char(c: u8) -> bool {
    match c {
        48..=57   // 0..9
        | 65..=90   // A..Z
        | 97..=122  // a..z
        | 45..=46   // -.
        | 95        // _
        => true,
        _ => false,
    }
}

/// Calls the `is_valid_name` function with the given name and panics if it returns `false`.
pub(crate) fn assert_valid_name<S: AsRef<str>>(name: S) {
    if name.as_ref().is_empty() {
        panic!("Index name must not be empty")
    }

    if !is_valid_index_name(name) {
        panic!("Wrong characters using in name. Use: a-zA-Z0-9 and _-.");
    }
}
