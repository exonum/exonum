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

//! Config manager interface.

use crate::node::ConnectListConfig;

/// Interface of the Config Manager usable for updating node configuration on
/// the fly.
pub trait ConfigManager: Send {
    /// Update connect list in the node configuration.
    fn store_connect_list(&mut self, connect_list: ConnectListConfig);
}
