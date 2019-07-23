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

/// Fast concatenation of byte arrays and/or keys that implements
/// `BinaryKey` trait.
///
/// ```
/// let prefix = vec![0_u8; 10];
/// let key = PublicKey::zero();
///
/// let _result = concat_keys!(prefix, key);
/// ```
macro_rules! concat_keys {
    (@capacity $key:expr) => ( $key.size() );
    (@capacity $key:expr, $($tail:expr),+) => (
        $key.size() + concat_keys!(@capacity $($tail),+)
    );
    ($($key:expr),+) => ({
        let capacity = concat_keys!(@capacity $($key),+);
        let mut buf = Vec::with_capacity(capacity);

        // Unsafe `set_len` here is safe because we never read from `buf`
        // before we write all elements to it.
        #[allow(unsafe_code)]
        unsafe {
            buf.set_len(capacity);
        }

        let mut _pos = 0;
        $(
            _pos += $key.write(&mut buf[_pos.._pos + $key.size()]);
        )*
        buf
    });
}

#[macro_export]
macro_rules! impl_object_hash_for_binary_value {
    ($( $type:ty ),*) => {
        $(
            impl ObjectHash for $type {
                fn object_hash(&self) -> Hash {
                    exonum_crypto::hash(&self.to_bytes())
                }
            }
        )*
    };
}
