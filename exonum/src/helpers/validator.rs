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

use exonum_crypto::PublicKey;
use exonum_merkledb::Snapshot;

use crate::{blockchain::Schema as CoreSchema, helpers::ValidatorId};

/// Attempts to find a `ValidatorId` by the provided service public key.
pub fn validator_id(snapshot: &dyn Snapshot, service_public_key: PublicKey) -> Option<ValidatorId> {
    CoreSchema::new(snapshot)
        .consensus_config()
        .find_validator(|validator_keys| service_public_key == validator_keys.service_key)
}
