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

//! Data migrations defined for the cryptocurrency service.

use exonum::crypto::Hash;
use exonum::runtime::{
    migrations::{
        InitMigrationError, LinearMigrations, MigrateData, MigrationContext, MigrationError,
        MigrationScript,
    },
    versioning::Version,
    CallerAddress,
};
use exonum_rust_runtime::ServiceFactory;

use crate::{wallet::Wallet, CryptocurrencyService, SchemaImpl};
use old_cryptocurrency::schema::{CurrencySchema as OldSchema, Wallet as OldWallet};

#[cfg(test)]
mod tests;

fn convert_wallet(old_wallet: OldWallet) -> Wallet {
    Wallet {
        owner: CallerAddress::from_key(old_wallet.pub_key),
        name: old_wallet.name,
        balance: old_wallet.balance,
        history_len: 0,
        history_hash: Hash::zero(),
    }
}

/// Migration script which takes wallets from the old storage, transforms them an save to
/// (a now Merkelized) `wallets` index.
pub fn migrate_wallets(context: &mut MigrationContext) -> Result<(), MigrationError> {
    /// Number of wallets to process between persisting intermediate migration results.
    const CHUNK_SIZE: usize = 100;

    context.helper.iter_loop(|helper, iters| {
        let old_schema = OldSchema::new(helper.old_data());
        let mut new_schema = SchemaImpl::new(helper.new_data());

        let wallets = iters.create("wallets", &old_schema.wallets);
        for (_, old_wallet) in wallets.take(CHUNK_SIZE) {
            let new_wallet = convert_wallet(old_wallet);
            let addr = new_wallet.owner;
            new_schema.public.wallets.put(&addr, new_wallet);
        }
    })?;
    Ok(())
}

impl MigrateData for CryptocurrencyService {
    fn migration_scripts(
        &self,
        start_version: &Version,
    ) -> Result<Vec<MigrationScript>, InitMigrationError> {
        // Declare an exhaustive list of scripts that transform the service data
        // to the corresponding version. Here, we have just one script.
        let latest_version = self.artifact_id().version;
        LinearMigrations::new(latest_version)
            .add_script(Version::new(0, 2, 0), migrate_wallets)
            .select(start_version)
    }
}
