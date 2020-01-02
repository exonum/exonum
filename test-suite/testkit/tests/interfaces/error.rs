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

use exonum_derive::ExecutionFail;

/// Common errors emitted by transactions during execution.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[derive(ExecutionFail)]
pub enum Error {
    /// Wallet not found.
    WalletNotFound = 0,
    /// Wallet already exists.
    WalletAlreadyExists = 1,
    /// Wrong interface caller.
    WrongInterfaceCaller = 2,
    /// Issuer is not authorized.
    UnauthorizedIssuer = 3,
}
