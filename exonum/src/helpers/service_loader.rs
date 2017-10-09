// Copyright 2017 The Exonum Team
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

//! Utilities for loading dynamic services.

use libloading::{Library, Symbol};

use std::error::Error;
use std::ffi::OsStr;

use super::fabric::ServiceFactory;

/// Provides interface for manipulating with dynamic services.
///
/// # Examples
///
/// ```no_run
/// use exonum::helpers::service_loader::DynamicServiceLoader;
///
/// let loader = DynamicServiceLoader::new("path_to_service_lib").unwrap();
/// let factory = loader.service_factory().unwrap();
/// # drop(factory);
/// ```
#[derive(Debug)]
pub struct DynamicServiceLoader {
    lib: Library,
}

impl DynamicServiceLoader {
    /// Creates a new `DynamicServiceLoader` instance associated with specified dynamic library.
    pub fn new<P: AsRef<OsStr>>(lib_path: P) -> Result<Self, Box<Error>> {
        let lib = Library::new(lib_path)?;
        Ok(Self { lib })
    }

    /// Returns a new `ServiceFactory` instance from the loaded library.
    ///
    /// Given library must export `create_service_factory` function.
    pub fn service_factory(&self) -> Result<Box<ServiceFactory>, Box<Error>> {
        let create_service_fn = b"create_service_factory";

        let create_factory: Symbol<fn() -> Box<ServiceFactory>> =
            unsafe { self.lib.get(create_service_fn)? };

        Ok(create_factory())
    }
}
