// version.h

extern {
    pub fn sodium_version_string() -> *const c_char;
    pub fn sodium_library_version_major() -> c_int;
    pub fn sodium_library_version_minor() -> c_int;
}

#[test]
fn test_sodium_library_version_major() {
    assert!(unsafe { sodium_library_version_major() } > 0)
}

#[test]
fn test_sodium_library_version_minor() {
    assert!(unsafe { sodium_library_version_minor() } >= 0)
}
