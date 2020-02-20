// Copyright 2020 The Exonum Team
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

//! Information about current node including Exonum, Rust and OS versions.

static USER_AGENT: &str = include_str!(concat!(env!("OUT_DIR"), "/user_agent"));

/// Returns "user agent" string containing information about Exonum, Rust and OS versions.
///
/// # Examples
///
/// ```
/// use exonum::helpers::user_agent;
///
/// let user_agent = user_agent();
/// println!("{}", user_agent);
/// ```
#[doc(hidden)]
pub fn user_agent() -> String {
    let os = os_info::get();
    format!("{}/{}", USER_AGENT, os)
}

/// Returns OS info of host on which run the node.
#[doc(hidden)]
pub fn os_info() -> String {
    os_info::get().to_string()
}

/// Returns a version of the exonum framework.
#[doc(hidden)]
pub fn exonum_version() -> Option<String> {
    let versions: Vec<_> = USER_AGENT.split('/').collect();
    versions.get(0).map(|v| v.trim().to_owned())
}

/// Returns a version of the rust compiler.
#[doc(hidden)]
pub fn rust_version() -> Option<String> {
    let versions: Vec<_> = USER_AGENT.split('/').collect();
    versions.get(1).map(|v| v.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Checks that user agent string contains three nonempty components.
    #[test]
    fn components() {
        let user_agent = user_agent();
        let components: Vec<_> = user_agent.split('/').collect();
        assert_eq!(components.len(), 3);

        for val in components {
            assert!(!val.is_empty());
        }
    }

    #[test]
    fn check_exonum_versions() {
        let exonum_version = exonum_version().unwrap();
        assert!(exonum_version.contains("exonum"));
    }

    #[test]
    fn check_rust_versions() {
        let rust_version = rust_version().unwrap();
        assert!(rust_version.contains("rust"));
    }
}
