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

//! The set of specific for the Rust runtime implementation errors.

use exonum_derive::ExecutionFail;

/// List of possible Rust runtime errors.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[derive(ExecutionFail)]
#[execution_fail(kind = "runtime")]
pub enum Error {
    /// Cannot deploy artifact because it has non-empty specification.
    IncorrectArtifactId = 0,
    /// Unable to deploy artifact with the specified identifier, it is not listed
    /// among available artifacts.
    UnableToDeploy = 1,
}
