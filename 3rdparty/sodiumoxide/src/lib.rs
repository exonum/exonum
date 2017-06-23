//! Rust bindings to the [sodium library](https://github.com/jedisct1/libsodium).
//!
//! Sodium is a portable implementation of Dan Bernsteins [NaCl: Networking and
//! Cryptography library](http://nacl.cr.yp.to)
//!
//! For most users, if you want public-key (asymmetric) cryptography you should use
//! the functions in `crypto::box_` for encryption/decryption.
//!
//! If you want secret-key (symmetric) cryptography you should be using the
//! functions in `crypto::secretbox` for encryption/decryption.
//!
//! For public-key signatures you should use the functions in `crypto::sign` for
//! signature creation and verification.
//!
//! Unless you know what you're doing you most certainly don't want to use the
//! functions in `crypto::scalarmult`, `crypto::stream`, `crypto::auth` and
//! `crypto::onetimeauth`.
//!
//! ## Thread Safety
//! All functions in this library are thread-safe provided that the `init()`
//! function has been called during program execution.
//!
//! If `init()` hasn't been called then all functions except the random-number
//! generation functions and the key-generation functions are thread-safe.
//!
//! # Public-key cryptography
//!  `crypto::box_`
//!
//!  `crypto::sign`
//!
//! # Sealed boxes
//!  `crypto::sealedox`
//!
//! # Secret-key cryptography
//!  `crypto::secretbox`
//!
//!  `crypto::stream`
//!
//!  `crypto::auth`
//!
//!  `crypto::onetimeauth`
//!
//! # Low-level functions
//!  `crypto::hash`
//!
//!  `crypto::verify`
//!
//!  `crypto::shorthash`
#![crate_name = "sodiumoxide"]
#![crate_type = "lib"]
#![warn(missing_docs)]
#![warn(non_upper_case_globals)]
#![warn(non_camel_case_types)]
#![warn(unused_qualifications)]

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), feature(alloc))]
#![cfg_attr(not(feature = "std"), feature(collections))]

extern crate libsodium_sys as ffi;
extern crate libc;
#[cfg(any(test, feature = "serde"))]
extern crate serde;
#[cfg(test)]
extern crate rustc_serialize;
#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
#[macro_use]
extern crate collections;
#[cfg(all(test, not(feature = "std")))]
extern crate std;

#[cfg(all(not(test), not(feature = "std")))]
mod std {
    pub use core::{cmp, fmt, hash, iter, mem, ops, slice, str};
}

#[cfg(not(feature = "std"))]
mod prelude {
    pub use collections::{Vec, String};
}

/// `init()` initializes the sodium library and chooses faster versions of
/// the primitives if possible. `init()` also makes the random number generation
/// functions (`gen_key`, `gen_keypair`, `gen_nonce`, `randombytes`, `randombytes_into`)
/// thread-safe
///
/// `init()` returns `false` if initialization failed.
pub fn init() -> bool {
    unsafe {
        ffi::sodium_init() != -1
    }
}

#[macro_use]
mod newtype_macros;
pub mod randombytes;
pub mod utils;
pub mod version;

#[cfg(test)]
mod test_utils;

/// Cryptographic functions
pub mod crypto {
    pub mod box_;
    pub mod sealedbox;
    pub mod sign;
    pub mod scalarmult;
    pub mod auth;
    pub mod hash;
    pub mod secretbox;
    pub mod onetimeauth;
    pub mod pwhash;
    pub mod stream;
    pub mod shorthash;
    pub mod verify;
}
