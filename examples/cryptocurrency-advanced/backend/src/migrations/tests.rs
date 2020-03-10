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

use exonum::{
    crypto::{self, KeyPair, Seed},
    helpers::Height,
    merkledb::access::{Access, RawAccessMut},
    runtime::{CallerAddress, InstanceId, SnapshotExt, SUPERVISOR_INSTANCE_ID as SUPERVISOR_ID},
};
use exonum_rust_runtime::{DefaultInstance, ServiceFactory};
use exonum_supervisor::{ConfigPropose, MigrationRequest, Supervisor, SupervisorInterface};
use exonum_testkit::{migrations::MigrationTest, Spec, TestKit, TestKitBuilder};
use rand::{rngs::StdRng, Rng, SeedableRng};

use std::{collections::HashMap, iter, thread, time::Duration};

use crate::{
    transactions::Transfer, CryptocurrencyInterface as New, CryptocurrencyService, Schema,
    SchemaImpl,
};
use old_cryptocurrency::{
    contracts::{CryptocurrencyInterface as Old, CryptocurrencyService as OldService},
    schema::{CurrencySchema as OldSchema, Wallet as OldWallet},
    transactions::{CreateWallet as OldCreate, TxTransfer as OldTransfer},
};

fn name_to_keypair(name: &str) -> KeyPair {
    let seed = crypto::hash(name.as_bytes());
    let seed = Seed::new(seed.as_bytes());
    KeyPair::from_seed(&seed)
}

fn prepare_wallets<'a, T>(fork: T, wallets: impl Iterator<Item = (&'a str, u64)>)
where
    T: Access,
    T::Base: RawAccessMut,
{
    let mut schema = OldSchema::new(fork);
    for (name, balance) in wallets {
        let pub_key = name_to_keypair(name).public_key();
        let wallet = OldWallet {
            pub_key,
            name: name.to_owned(),
            balance,
        };
        schema.wallets.put(&pub_key, wallet);
    }
}

fn assert_state<'a>(
    schema: &SchemaImpl<impl Access>,
    wallets: impl Iterator<Item = (&'a str, u64)>,
) {
    let mut expected_wallet_count = 0;
    for (name, balance) in wallets {
        let pub_key = name_to_keypair(name).public_key();
        let addr = CallerAddress::from_key(pub_key);
        let wallet = schema.public.wallets.get(&addr).unwrap_or_else(|| {
            panic!("Wallet for user `{}` was not transformed", name);
        });
        expected_wallet_count += 1;

        assert_eq!(wallet.name, name);
        assert_eq!(wallet.balance, balance);
        assert_eq!(wallet.owner, addr);
    }

    assert_eq!(schema.public.wallets.iter().count(), expected_wallet_count);
}

#[test]
fn isolated_test_with_handwritten_data() {
    let wallets = &[("alice", 75), ("bob", 120), ("carol", 3)];

    let old_version = OldService.artifact_id().version;
    let mut test = MigrationTest::new(CryptocurrencyService, old_version);
    test.setup(|fork| prepare_wallets(fork, wallets.iter().copied()));

    let schema = SchemaImpl::new(test.migrate().end_snapshot());
    assert_state(&schema, wallets.iter().copied());
}

fn generate_random_wallets() -> impl Iterator<Item = (String, u64)> {
    const RNG_SEED: u64 = 123_456_789;

    let mut rng = StdRng::seed_from_u64(RNG_SEED);
    iter::from_fn(move || {
        let name = format!("User #{}", rng.gen::<u32>());
        let balance = rng.gen_range(0, 1_000);
        Some((name, balance))
    })
}

#[test]
fn isolated_test_with_random_data() {
    // We don't want duplicate users, hence the use of `HashMap`.
    const WALLET_COUNT: usize = 1_234;
    let wallets: HashMap<_, _> = generate_random_wallets().take(WALLET_COUNT).collect();
    let wallets_iter = wallets
        .iter()
        .map(|(name, balance)| (name.as_str(), *balance));

    let old_version = OldService.artifact_id().version;
    let mut test = MigrationTest::new(CryptocurrencyService, old_version);
    test.setup(|fork| {
        prepare_wallets(fork, wallets_iter.clone());
    });

    let schema = SchemaImpl::new(test.migrate().end_snapshot());
    assert_state(&schema, wallets_iter);
}

fn init_testkit() -> TestKit {
    TestKitBuilder::validator()
        // Add old version of the service.
        .with(Spec::new(OldService).with_default_instance())
        // Add the artifact for the new version.
        .with(Spec::migrating(CryptocurrencyService))
        // Add the supervisor service.
        .with(Supervisor::simple())
        .build()
}

#[test]
fn test_with_full_lifecycle() {
    const SERVICE_ID: InstanceId = OldService::INSTANCE_ID;
    let mut testkit = init_testkit();

    // Create some accounts using the old service.
    let alice = KeyPair::random();
    let bob = KeyPair::random();
    let carol = KeyPair::random();
    let txs = vec![
        Old::create_wallet(&alice, SERVICE_ID, OldCreate::new("alice")),
        Old::create_wallet(&bob, SERVICE_ID, OldCreate::new("bob")),
        Old::create_wallet(&carol, SERVICE_ID, OldCreate::new("carol")),
    ];
    let block = testkit.create_block_with_transactions(txs);
    assert!(block.errors.is_empty());

    // Transfer value between two accounts.
    let transfer = OldTransfer {
        to: bob.public_key(),
        amount: 16,
        seed: 0,
    };
    let transfer = Old::transfer(&alice, SERVICE_ID, transfer);
    let block = testkit.create_block_with_transaction(transfer);
    assert!(block.errors.is_empty());

    // Migrate the service. To do this, we first need to stop or freeze the service.
    let proposal = ConfigPropose::immediate(0).freeze_service(SERVICE_ID);
    let admin_keys = testkit.us().service_keypair();
    let freeze_service = admin_keys.propose_config_change(SUPERVISOR_ID, proposal);
    let block = testkit.create_block_with_transaction(freeze_service);
    assert!(block.errors.is_empty());

    // Then, we can launch the migration.
    let migration_req = MigrationRequest {
        new_artifact: CryptocurrencyService.artifact_id(),
        service: OldService::INSTANCE_NAME.to_owned(),
        deadline_height: Height(100),
    };
    let migration = admin_keys.request_migration(SUPERVISOR_ID, migration_req.clone());
    let block = testkit.create_block_with_transaction(migration);
    assert!(block.errors.is_empty());

    // Since migration is performed in a background, it may take a little while.
    // Thus, we `sleep` and then create a couple of blocks, which are necessary to finalize
    // the migration.
    thread::sleep(Duration::from_millis(50));
    for _ in 0..3 {
        let block = testkit.create_block();
        assert!(block.errors.is_empty());
    }

    // Check that the migration is completed.
    let snapshot = testkit.snapshot();
    let service_state = snapshot.for_dispatcher().get_instance(SERVICE_ID).unwrap();
    let new_version = CryptocurrencyService.artifact_id().version;
    assert_eq!(*service_state.data_version(), new_version);

    // Reassign the service artifact and resume it.
    let migration_req = MigrationRequest {
        deadline_height: testkit.height(),
        ..migration_req
    };
    let resume_service = ConfigPropose::immediate(1).resume_service(SERVICE_ID, ());
    let block = testkit.create_block_with_transactions(vec![
        admin_keys.request_migration(SUPERVISOR_ID, migration_req),
        admin_keys.propose_config_change(SUPERVISOR_ID, resume_service),
    ]);
    assert!(block.errors.is_empty());

    // Check that users can now transfer their tokens!
    let transfer = Transfer {
        to: CallerAddress::from_key(carol.public_key()),
        amount: 4,
        seed: 0,
    };
    let transfer = New::transfer(&alice, SERVICE_ID, transfer);
    let block = testkit.create_block_with_transaction(transfer);
    block[0].status().unwrap();

    let snapshot = testkit.snapshot();
    let schema: Schema<_> = snapshot.service_schema(SERVICE_ID).unwrap();
    let alice_addr = CallerAddress::from_key(alice.public_key());
    let alice_wallet = schema.wallets.get(&alice_addr).unwrap();
    assert_eq!(alice_wallet.balance, 80); // 16 tokens sent to Bob and 4 to Carol.
    assert_eq!(alice_wallet.history_len, 1);
}
