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

use chrono::{DateTime, Utc};

use crate::{proto::{schema::consensus}, crypto::{PublicKey, Signature}};

/// Container for the signed messages.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[exonum(pb = "consensus::Signed", crate = "crate")]
pub struct Signed {
    /// Message payload.
    pub(in super) payload: Vec<u8>,
    /// Message author.
    pub(in super) author: PublicKey,
    /// Digital signature.
    pub(in super) signature: Signature,
}

/// Connect to a node.
///
/// ### Validation
/// The message is ignored if its time is earlier than in the previous
/// `Connect` message received from the same peer.
///
/// ### Processing
/// Connect to the peer.
///
/// ### Conditions 
/// A node sends `Connect` message to all known addresses during
/// initialization. Additionally, the node responds by its own `Connect`
/// message after receiving `node::Event::Connected`.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[exonum(pb = "consensus::Connect", crate = "crate")]
pub struct Connect {
    /// The node's address.
    pub host: String,
    /// Time when the message was created.
    pub time: DateTime<Utc>,
    /// String containing information about this node including Exonum, Rust and OS versions.
    pub user_agent: String,
}
