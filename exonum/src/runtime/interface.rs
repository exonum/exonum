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

//! Service interface schema definition.

use super::MethodId;

/// Structure describing the interface method: its numeric ID, human-readable name
/// and expected input as protobuf message path.
#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceMethod {
    /// Numeric identifier of the method.
    pub id: MethodId,
    /// Name of the method.
    pub name: String,
    /// Input parameter type.
    /// It should be a fully-qualified protobuf
    /// message name, e.g. `exonum.supervisor.DeployRequest`.
    pub input: String,
}

/// Trait denoting the interface as the sequence of method descriptions.
pub trait InterfaceLayout {
    /// Name of the interface.
    fn name(&self) -> String;

    /// A list of methods that interface should implement.
    fn methods(&self) -> Vec<InterfaceMethod>;
}
