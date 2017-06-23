macro_rules! hash_module (($hash_name:ident,
                           $hashbytes:expr,
                           $blockbytes:expr,
                           $state_name:ident,
                           $init_name:ident,
                           $update_name:ident,
                           $final_name:ident) => (

use libc::c_ulonglong;
use std::mem;
use std::fmt;

/// Number of bytes in a `Digest`.
pub const DIGESTBYTES: usize = $hashbytes;

/// Block size of the hash function.
pub const BLOCKBYTES: usize = $blockbytes;

new_type! {
    /// Digest-structure
    public Digest(DIGESTBYTES);
}

/// `hash` hashes a message `m`. It returns a hash `h`.
pub fn hash(m: &[u8]) -> Digest {
    unsafe {
        let mut h = [0; DIGESTBYTES];
        $hash_name(&mut h, m.as_ptr(), m.len() as c_ulonglong);
        Digest(h)
    }
}

// Streaming hashing.
// Mostly follows the streaming HMAC interface and implementation.

// `Clone` may be used to speed up, e.g., hashing many messages that
// have a common prefix. It also makes `finalize` moving `State` less
// inconvenient.

/// State for multi-part (streaming) computation of hash digest.
#[derive(Clone)]
pub struct State($state_name);

impl State {
    /// `init()` initialize a streaming hashing state.
    pub fn init() -> State {
        unsafe {
            let mut s = mem::uninitialized();
            $init_name(&mut s);
            State(s)
        }
    }

    /// `update()` can be called more than once in order to compute the digest
    /// from sequential chunks of the message.
    pub fn update(&mut self, in_: &[u8]) {
        unsafe {
            $update_name(&mut self.0, in_.as_ptr(), in_.len() as c_ulonglong);
        }
    }

    /// `finalize()` finalizes the hashing computation and returns a `Digest`.

    // Moves self becuase libsodium says the state should not be used
    // anymore after final().
    pub fn finalize(mut self) -> Digest {
        unsafe {
            let mut digest = [0; $hashbytes as usize];
            $final_name(&mut self.0, &mut digest);
            Digest(digest)
        }
    }
}

// Impl Default becuase `State` does have a sensible default: State::init()
impl Default for State {
    fn default() -> State {
        State::init()
    }
}

impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "hash_sha256 state")
    }
}


#[cfg(feature = "default")]
#[cfg(test)]
mod test_encode {
    use super::*;
    use test_utils::round_trip;

    #[test]
    fn test_serialisation() {
        use randombytes::randombytes;
        for i in 0..32usize {
            let m = randombytes(i);
            let d = hash(&m[..]);
            round_trip(d);
        }
    }
}

#[cfg(feature = "benchmarks")]
#[cfg(test)]
mod bench_m {
    extern crate test;
    use randombytes::randombytes;
    use super::*;

    const BENCH_SIZES: [usize; 14] = [0, 1, 2, 4, 8, 16, 32, 64,
                                      128, 256, 512, 1024, 2048, 4096];

    #[bench]
    fn bench_hash(b: &mut test::Bencher) {
        let ms: Vec<Vec<u8>> = BENCH_SIZES.iter().map(|s| {
            randombytes(*s)
        }).collect();
        b.iter(|| {
            for m in ms.iter() {
                hash(&m);
            }
        });
    }
}

));