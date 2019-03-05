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

/// Config for Configuration service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigurationServiceConfig {
    /// Number of votes required to commit the new configuration.
    /// This value should be greater than 2/3 and less or equal to the
    /// validators count.
    pub majority_count: Option<u16>,
}

impl Default for ConfigurationServiceConfig {
    fn default() -> Self {
        Self {
            majority_count: None,
        }
    }
}
