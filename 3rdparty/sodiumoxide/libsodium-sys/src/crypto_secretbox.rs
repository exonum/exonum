pub const crypto_secretbox_KEYBYTES: usize = 32;
pub const crypto_secretbox_NONCEBYTES: usize = 24;
pub const crypto_secretbox_MACBYTES: usize = 16;
pub const crypto_secretbox_PRIMITIVE: &'static str = "xsalsa20poly1305";


extern {
    pub fn crypto_secretbox_easy(
        c: *mut u8,
        m: *const u8,
        mlen: c_ulonglong,
        n: *const [u8; crypto_secretbox_NONCEBYTES],
        k: *const [u8; crypto_secretbox_KEYBYTES]) -> c_int;
    pub fn crypto_secretbox_open_easy(
        m: *mut u8,
        c: *const u8,
        clen: c_ulonglong,
        n: *const [u8; crypto_secretbox_NONCEBYTES],
        k: *const [u8; crypto_secretbox_KEYBYTES]) -> c_int;
    pub fn crypto_secretbox_detached(
        c: *mut u8,
        mac: *mut [u8; crypto_secretbox_MACBYTES],
        m: *const u8,
        mlen: c_ulonglong,
        n: *const [u8; crypto_secretbox_NONCEBYTES],
        k: *const [u8; crypto_secretbox_KEYBYTES]) -> c_int;
    pub fn crypto_secretbox_open_detached(
        m: *mut u8,
        c: *const u8,
        mac: *const [u8; crypto_secretbox_MACBYTES],
        clen: c_ulonglong,
        n: *const [u8; crypto_secretbox_NONCEBYTES],
        k: *const [u8; crypto_secretbox_KEYBYTES]) -> c_int;
    pub fn crypto_secretbox_keybytes() -> size_t;
    pub fn crypto_secretbox_noncebytes() -> size_t;
    pub fn crypto_secretbox_macbytes() -> size_t;
    pub fn crypto_secretbox_primitive() -> *const c_char;
}


#[test]
fn test_crypto_secretbox_keybytes() {
    assert!(unsafe {
        crypto_secretbox_keybytes() as usize
    } == crypto_secretbox_KEYBYTES)
}

#[test]
fn test_crypto_secretbox_noncebytes() {
    assert!(unsafe {
        crypto_secretbox_noncebytes() as usize
    } == crypto_secretbox_NONCEBYTES)
}

#[test]
fn test_crypto_secretbox_macbytes() {
    assert!(unsafe {
        crypto_secretbox_macbytes() as usize
    } == crypto_secretbox_MACBYTES)
}

#[test]
fn test_crypto_secretbox_primitive() {
    unsafe {
        let s = crypto_secretbox_primitive();
        let s = std::ffi::CStr::from_ptr(s).to_bytes();
        assert!(s == crypto_secretbox_PRIMITIVE.as_bytes());
    }
}
