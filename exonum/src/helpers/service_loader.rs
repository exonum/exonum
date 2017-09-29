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

const CREATE_SERVICE_FN: &[u8; 22] = b"create_service_factory";

/// Provides interface for manipulating with dynamic services.
#[derive(Debug)]
pub struct DynamicServiceLoader {}

impl DynamicServiceLoader {
    /// Loads dynamic library by path and returns `ServiceFactory`.
    ///
    /// Given library must export `create_service_factory` function.
    pub fn load<P: AsRef<OsStr>>(lib_path: P) -> Result<Box<ServiceFactory>, Box<Error>> {
        let lib = Library::new(lib_path)?;
        let create_factory: Symbol<fn() -> Box<ServiceFactory>> =
            unsafe { lib.get(CREATE_SERVICE_FN)? };
        Ok(create_factory())
    }
}
