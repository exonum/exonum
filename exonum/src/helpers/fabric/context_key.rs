// Copyright 2018 The Exonum Team
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

use std::{fmt, marker::PhantomData};

/// `ContextKey` provides strongly typed access to data inside `Context`.
/// See `exonum::fabric::keys` for keys used by the exonum itself.
pub struct ContextKey<T> {
    // These fields are public so that `context_key`
    // macro works outside of this crate. It should be
    // replaced with `const fn`, once it is stable.
    #[doc(hidden)]
    pub __name: &'static str,
    #[doc(hidden)]
    pub __phantom: PhantomData<T>,
}

// We need explicit `impl Copy` because derive won't work if `T: !Copy`.
impl<T> Copy for ContextKey<T> {}

impl<T> Clone for ContextKey<T> {
    fn clone(&self) -> Self {
        Self {
            __name: self.__name,
            __phantom: self.__phantom,
        }
    }
}

impl<T> fmt::Debug for ContextKey<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "ContextKey({:?})", self.__name)
    }
}

impl<T> ContextKey<T> {
    /// Name of this key.
    pub fn name(&self) -> &str {
        self.__name
    }
}

/// Constructs a `ContextKey` from a given name.
///
/// For additional information refer to
/// [`exonum:helpers:fabric:ContextKey`].
///
/// [`exonum:helpers:fabric:ContextKey`]: ./helpers/fabric/struct.ContextKey.html
///
/// # Examples
///
/// The example below creates a constant using the `ContextKey` macro.
///
/// ```
/// #[macro_use]
/// extern crate exonum;
/// use exonum::helpers::fabric::ContextKey;
///
/// const NAME: ContextKey<String> = context_key!("name");
/// # fn main() {}
/// ```
#[macro_export]
macro_rules! context_key {
    ($name:expr) => {{
        $crate::helpers::fabric::ContextKey {
            __name: $name,
            __phantom: ::std::marker::PhantomData,
        }
    }};
}

#[test]
fn key_is_copy() {
    const K: ContextKey<Vec<String>> = context_key!("k");
    let x = K;
    let y = x;
    assert_eq!(x.name(), y.name());
}
