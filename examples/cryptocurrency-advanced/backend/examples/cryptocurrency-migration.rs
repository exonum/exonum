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

use exonum_cli::NodeBuilder;

use exonum_cryptocurrency_advanced::CryptocurrencyService;
use old_cryptocurrency::contracts::CryptocurrencyService as OldService;

fn main() -> Result<(), failure::Error> {
    exonum::helpers::init_logger()?;
    NodeBuilder::new()
        .with_default_rust_service(OldService)
        .with_rust_service(CryptocurrencyService)
        .run()
}
