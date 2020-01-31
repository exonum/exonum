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
    blockchain::{config::InstanceInitParams, Blockchain, BlockchainBuilder, BlockchainMut},
    crypto::{gen_keypair_from_seed, hash, PublicKey, SecretKey, Seed},
    helpers::Height,
    runtime::{
        migrations::{InitMigrationError, InstanceMigration, MigrateData, MigrationScript},
        versioning::Version,
        ArtifactId, CoreError, ErrorMatch, ExecutionError, InstanceStatus, SnapshotExt,
        SUPERVISOR_INSTANCE_ID,
    },
};
use exonum_derive::*;
use exonum_rust_runtime::{DefaultInstance, RustRuntimeBuilder, ServiceFactory};
use exonum_supervisor::Supervisor;
use pretty_assertions::assert_eq;

use std::borrow::Cow;

use self::inspected::{
    create_block_with_transactions, create_genesis_config_builder, execute_transaction,
    EventsHandle, Inspected, MigrateService, ResumeService, RuntimeEvent, StopService,
    ToySupervisor, ToySupervisorService,
};

mod inspected;

#[derive(Debug, Clone)]
struct TestUser {
    full_name: Cow<'static, str>,
    first_name: Cow<'static, str>,
    last_name: Cow<'static, str>,
    balance: u64,
}

const USERS: &[TestUser] = &[
    TestUser {
        full_name: Cow::Borrowed("Deep Thought"),
        first_name: Cow::Borrowed("Deep"),
        last_name: Cow::Borrowed("Thought"),
        balance: 42,
    },
    TestUser {
        full_name: Cow::Borrowed("Arthur Dent"),
        first_name: Cow::Borrowed("Arthur"),
        last_name: Cow::Borrowed("Dent"),
        balance: 7,
    },
    TestUser {
        full_name: Cow::Borrowed("Trillian"),
        first_name: Cow::Borrowed("Trillian"),
        last_name: Cow::Borrowed(""),
        balance: 90,
    },
    TestUser {
        full_name: Cow::Borrowed("Marvin \"The Paranoid\" Android"),
        first_name: Cow::Borrowed("Marvin \"The Paranoid\""),
        last_name: Cow::Borrowed("Android"),
        balance: 0,
    },
];

impl TestUser {
    fn keypair(&self) -> (PublicKey, SecretKey) {
        let seed = hash(self.full_name.as_bytes());
        let seed = Seed::from_slice(&seed[..]).unwrap();
        gen_keypair_from_seed(&seed)
    }
}

#[exonum_interface(auto_ids)]
trait Migration<Ctx> {
    type Output;

    fn op(&self, context: Ctx, arg: ()) -> Self::Output;
}

mod v01 {
    use exonum::{
        crypto::PublicKey,
        merkledb::{
            access::{Access, FromAccess, Prefixed},
            Fork, MapIndex,
        },
        runtime::{ExecutionContext, ExecutionError},
    };
    use exonum_derive::*;
    use exonum_derive::{BinaryValue, FromAccess, ObjectHash};
    use exonum_rust_runtime::{DefaultInstance, Service, ServiceFactory};
    use serde_derive::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, Serialize, Deserialize)]
    #[derive(BinaryValue, ObjectHash)]
    #[binary_value(codec = "bincode")]
    pub struct Wallet {
        pub username: String,
        pub balance: u64,
    }

    #[derive(Debug, FromAccess, RequireArtifact)]
    pub struct Schema<T: Access> {
        pub wallets: MapIndex<T::Base, PublicKey, Wallet>,
    }

    impl<T: Access> Schema<T> {
        pub fn new(access: T) -> Self {
            Self::from_root(access).unwrap()
        }
    }

    pub(crate) fn generate_test_data(access: Prefixed<&Fork>, users: &[TestUser]) {
        let mut schema = Schema::new(access);

        for user in users {
            let (key, _) = user.keypair();
            let wallet = Wallet {
                username: user.full_name.to_string(),
                balance: user.balance,
            };
            schema.wallets.put(&key, wallet);
        }
    }

    #[derive(Debug, ServiceFactory, ServiceDispatcher)]
    #[service_dispatcher(implements("Migration"))]
    #[service_factory(artifact_name = "migration", artifact_version = "0.1.0")]
    pub struct MigrationService;

    impl Migration<ExecutionContext<'_>> for MigrationService {
        type Output = Result<(), ExecutionError>;

        fn op(&self, _context: ExecutionContext<'_>, _arg: ()) -> Self::Output {
            Ok(())
        }
    }

    impl Service for MigrationService {
        fn initialize(
            &self,
            context: ExecutionContext<'_>,
            _params: Vec<u8>,
        ) -> Result<(), ExecutionError> {
            let data = context.service_data();
            generate_test_data(data, USERS);
            Ok(())
        }
    }

    impl DefaultInstance for MigrationService {
        const INSTANCE_ID: u32 = 2;
        const INSTANCE_NAME: &'static str = "migration";

        fn default_instance(&self) -> InstanceInitParams {
            self.artifact_id()
                .into_default_instance(Self::INSTANCE_ID, Self::INSTANCE_NAME)
            // .with_constructor(INITIAL_BALANCE)
        }
    }

    impl MigrateData for MigrationService {
        fn migration_scripts(
            &self,
            _start_version: &Version,
        ) -> Result<Vec<MigrationScript>, InitMigrationError> {
            Ok(Vec::new())
        }
    }
}

mod v02 {
    use exonum::{
        crypto::PublicKey,
        merkledb::{
            access::{Access, FromAccess, Prefixed},
            Fork, MapIndex,
        },
        runtime::{
            migrations::{InitMigrationError, MigrateData, MigrationScript},
            versioning::Version,
            ExecutionContext, ExecutionError,
        },
    };
    use exonum_derive::*;
    use exonum_derive::{BinaryValue, FromAccess, ObjectHash};
    use exonum_rust_runtime::{DefaultInstance, Service, ServiceFactory};
    use serde_derive::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, Serialize, Deserialize)]
    #[derive(BinaryValue, ObjectHash)]
    #[binary_value(codec = "bincode")]
    pub struct Wallet {
        pub username: String,
        pub balance: u64,
    }

    #[derive(Debug, FromAccess, RequireArtifact)]
    pub struct Schema<T: Access> {
        pub wallets: MapIndex<T::Base, PublicKey, Wallet>,
    }

    impl<T: Access> Schema<T> {
        pub fn new(access: T) -> Self {
            Self::from_root(access).unwrap()
        }
    }

    pub(crate) fn generate_test_data(access: Prefixed<&Fork>, users: &[TestUser]) {
        let mut schema = Schema::new(access);

        for user in users {
            let (key, _) = user.keypair();
            let wallet = Wallet {
                username: user.full_name.to_string(),
                balance: user.balance,
            };
            schema.wallets.put(&key, wallet);
        }
    }

    #[derive(Debug, ServiceFactory, ServiceDispatcher)]
    #[service_dispatcher(implements("Migration"))]
    #[service_factory(artifact_name = "migration", artifact_version = "0.2.0")]
    pub struct MigrationService;

    impl Migration<ExecutionContext<'_>> for MigrationService {
        type Output = Result<(), ExecutionError>;

        fn op(&self, _context: ExecutionContext<'_>, _arg: ()) -> Self::Output {
            Ok(())
        }
    }

    impl Service for MigrationService {
        fn initialize(
            &self,
            context: ExecutionContext<'_>,
            _params: Vec<u8>,
        ) -> Result<(), ExecutionError> {
            let data = context.service_data();
            generate_test_data(data, USERS);
            Ok(())
        }
    }

    impl DefaultInstance for MigrationService {
        const INSTANCE_ID: u32 = 2;
        const INSTANCE_NAME: &'static str = "migration";

        fn default_instance(&self) -> InstanceInitParams {
            self.artifact_id()
                .into_default_instance(Self::INSTANCE_ID, Self::INSTANCE_NAME)
            // .with_constructor(INITIAL_BALANCE)
        }
    }

    impl MigrateData for MigrationService {
        fn migration_scripts(
            &self,
            _start_version: &Version,
        ) -> Result<Vec<MigrationScript>, InitMigrationError> {
            Ok(Vec::new())
        }
    }
}

mod v03 {
    use exonum::{
        crypto::PublicKey,
        merkledb::{
            access::{Access, FromAccess, Prefixed},
            Fork, MapIndex,
        },
        runtime::{ExecutionContext, ExecutionError},
    };
    use exonum_derive::*;
    use exonum_derive::{BinaryValue, FromAccess, ObjectHash};
    use exonum_rust_runtime::{DefaultInstance, Service, ServiceFactory};
    use serde_derive::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, Serialize, Deserialize)]
    #[derive(BinaryValue, ObjectHash)]
    #[binary_value(codec = "bincode")]
    pub struct Wallet {
        pub username: String,
        pub balance: u64,
    }

    #[derive(Debug, FromAccess, RequireArtifact)]
    pub struct Schema<T: Access> {
        pub wallets: MapIndex<T::Base, PublicKey, Wallet>,
    }

    impl<T: Access> Schema<T> {
        pub fn new(access: T) -> Self {
            Self::from_root(access).unwrap()
        }
    }

    pub(crate) fn generate_test_data(access: Prefixed<&Fork>, users: &[TestUser]) {
        let mut schema = Schema::new(access);

        for user in users {
            let (key, _) = user.keypair();
            let wallet = Wallet {
                username: user.full_name.to_string(),
                balance: user.balance,
            };
            schema.wallets.put(&key, wallet);
        }
    }

    #[derive(Debug, ServiceFactory, ServiceDispatcher)]
    #[service_dispatcher(implements("Migration"))]
    #[service_factory(artifact_name = "migration", artifact_version = "0.3.0")]
    pub struct MigrationService;

    impl Migration<ExecutionContext<'_>> for MigrationService {
        type Output = Result<(), ExecutionError>;

        fn op(&self, _context: ExecutionContext<'_>, _arg: ()) -> Self::Output {
            Ok(())
        }
    }

    impl Service for MigrationService {
        fn initialize(
            &self,
            context: ExecutionContext<'_>,
            _params: Vec<u8>,
        ) -> Result<(), ExecutionError> {
            let data = context.service_data();
            generate_test_data(data, USERS);
            Ok(())
        }
    }

    impl DefaultInstance for MigrationService {
        const INSTANCE_ID: u32 = 2;
        const INSTANCE_NAME: &'static str = "migration";

        fn default_instance(&self) -> InstanceInitParams {
            self.artifact_id()
                .into_default_instance(Self::INSTANCE_ID, Self::INSTANCE_NAME)
        }
    }
}

mod v04 {
    use exonum::{
        crypto::PublicKey,
        merkledb::{
            access::{Access, FromAccess, Prefixed},
            ProofEntry, ProofMapIndex, Snapshot,
        },
        runtime::{
            migrations::{MigrateData, MigrationContext, MigrationError, MigrationScript},
            versioning::Version,
            ExecutionContext, ExecutionError,
        },
    };
    use exonum_derive::FromAccess;
    use exonum_derive::*;
    use exonum_rust_runtime::{DefaultInstance, Service, ServiceFactory};

    use super::{
        v01, InitMigrationError, InstanceInitParams, Migration, ServiceDispatcher, TestUser,
    };
    // use super::*;

    // #[derive(Debug, Serialize, Deserialize)]
    // #[derive(BinaryValue, ObjectHash)]
    // #[binary_value(codec = "bincode")]
    // pub struct Wallet {
    //     pub username: String,
    //     pub balance: u64,
    // }

    #[derive(Debug, FromAccess, RequireArtifact)]
    pub struct Schema<T: Access> {
        pub wallets: ProofMapIndex<T::Base, PublicKey, v01::Wallet>,
        pub total_balance: ProofEntry<T::Base, u64>,
    }

    impl<T: Access> Schema<T> {
        pub fn new(access: T) -> Self {
            Self::from_root(access).unwrap()
        }
    }

    #[derive(Debug, ServiceFactory, ServiceDispatcher)]
    #[service_dispatcher(implements("Migration"))]
    #[service_factory(artifact_name = "migration", artifact_version = "0.4.0")]
    pub struct MigrationService;

    impl Migration<ExecutionContext<'_>> for MigrationService {
        type Output = Result<(), ExecutionError>;

        fn op(&self, _context: ExecutionContext<'_>, _arg: ()) -> Self::Output {
            Ok(())
        }
    }

    impl Service for MigrationService {
        fn initialize(
            &self,
            _context: ExecutionContext<'_>,
            _params: Vec<u8>,
        ) -> Result<(), ExecutionError> {
            Ok(())
        }
    }

    impl DefaultInstance for MigrationService {
        const INSTANCE_ID: u32 = 2;
        const INSTANCE_NAME: &'static str = "migration";

        fn default_instance(&self) -> InstanceInitParams {
            self.artifact_id()
                .into_default_instance(Self::INSTANCE_ID, Self::INSTANCE_NAME)
        }
    }

    impl MigrateData for MigrationService {
        fn migration_scripts(
            &self,
            _start_version: &Version,
        ) -> Result<Vec<MigrationScript>, InitMigrationError> {
            println!("v04::MigrationService::migration_scripts.");

            Ok(vec![MigrationScript::new(
                migrate_wallets,
                "0.4.0".parse().unwrap(),
            )])
        }
    }

    fn migrate_wallets(ctx: &mut MigrationContext) -> Result<(), MigrationError> {
        println!("Started wallet migration.");
        let old_schema = v01::Schema::new(ctx.helper.old_data());
        let mut new_schema = self::Schema::new(ctx.helper.new_data());

        let mut total_balance = 0;
        for (key, wallet) in &old_schema.wallets {
            total_balance += wallet.balance;
            new_schema.wallets.put(&key, wallet);
        }
        new_schema.total_balance.set(total_balance);
        println!("Finished wallet migration.");
        Ok(())
    }

    pub(crate) fn verify_schema(snapshot: Prefixed<&dyn Snapshot>, users: &[TestUser]) {
        let schema = Schema::new(snapshot);
        for user in users {
            let (key, _) = user.keypair();
            let wallet = schema.wallets.get(&key).unwrap();
            assert_eq!(wallet.balance, user.balance);
            assert_eq!(wallet.username, user.full_name);
        }
        assert_eq!(schema.wallets.iter().count(), users.len());

        let total_balance = schema.total_balance.get().unwrap();
        assert_eq!(
            total_balance,
            users.iter().map(|user| user.balance).sum::<u64>()
        );
    }
}

fn create_runtime() -> (BlockchainMut, EventsHandle) {
    let blockchain = Blockchain::build_for_tests();
    let genesis_config = create_genesis_config_builder()
        .with_artifact(ToySupervisorService.artifact_id())
        .with_instance(ToySupervisorService.default_instance())
        .with_artifact(v01::MigrationService.artifact_id())
        .with_instance(v01::MigrationService.default_instance())
        .with_artifact(v02::MigrationService.artifact_id())
        .with_artifact(v03::MigrationService.artifact_id())
        .with_artifact(v04::MigrationService.artifact_id())
        .build();

    let inspected = Inspected::new(
        RustRuntimeBuilder::new()
            .with_factory(ToySupervisorService)
            .with_factory(v01::MigrationService)
            .with_migrating_factory(v02::MigrationService)
            .with_factory(v03::MigrationService)
            .with_migrating_factory(v04::MigrationService)
            .build_for_tests(),
    );
    let events_handle = inspected.events.clone();

    let blockchain = BlockchainBuilder::new(blockchain, genesis_config)
        .with_runtime(inspected)
        .build();
    (blockchain, events_handle)
}

fn create_runtime_custom(
    initial: impl ServiceFactory + DefaultInstance,
    migrating: Vec<impl ServiceFactory + MigrateData>,
    not_migrating: Vec<impl ServiceFactory>,
) -> (BlockchainMut, EventsHandle) {
    let blockchain = Blockchain::build_for_tests();
    let mut genesis_config = create_genesis_config_builder()
        .with_artifact(ToySupervisorService.artifact_id())
        .with_instance(ToySupervisorService.default_instance())
        .with_artifact(initial.artifact_id())
        .with_instance(initial.default_instance());
    for it in &migrating {
        genesis_config = genesis_config.with_artifact(it.artifact_id());
    }
    for it in &not_migrating {
        genesis_config = genesis_config.with_artifact(it.artifact_id());
    }
    let genesis_config = genesis_config.build();

    let mut runtime = RustRuntimeBuilder::new()
        .with_factory(ToySupervisorService)
        .with_factory(initial);
    for it in migrating {
        runtime = runtime.with_migrating_factory(it);
    }
    for it in not_migrating {
        runtime = runtime.with_factory(it);
    }
    let runtime = runtime.build_for_tests();

    let inspected = Inspected::new(runtime);
    let events_handle = inspected.events.clone();

    let blockchain = BlockchainBuilder::new(blockchain, genesis_config)
        .with_runtime(inspected)
        .build();
    (blockchain, events_handle)
}

#[test]
fn fast_forward_migration() {
    let (mut blockchain, events_handle) = create_runtime();
    let keypair = blockchain.as_ref().service_keypair().clone();

    // Stop running service instance.
    execute_transaction(
        &mut blockchain,
        keypair.stop_service(
            ToySupervisorService::INSTANCE_ID,
            StopService {
                instance_id: v01::MigrationService::INSTANCE_ID,
            },
        ),
    )
    .unwrap();
    // We not interested in events in this case.
    drop(events_handle.take());

    // Make fast-forward migration to the v02::MigrationService.
    execute_transaction(
        &mut blockchain,
        keypair.migrate_service(
            ToySupervisorService::INSTANCE_ID,
            MigrateService {
                instance_name: v01::MigrationService::INSTANCE_NAME.to_owned(),
                artifact: v02::MigrationService.artifact_id(),
            },
        ),
    )
    .unwrap();

    let migration_service = v02::MigrationService.default_instance().instance_spec;
    assert_eq!(
        events_handle.take(),
        vec![
            RuntimeEvent::BeforeTransactions(Height(2), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::MigrateService(
                migration_service.artifact.clone(),
                v01::MigrationService.artifact_id().version
            ),
            RuntimeEvent::AfterTransactions(Height(2), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::AfterCommit(Height(3)),
        ]
    );

    // Resume stopped service instance.
    execute_transaction(
        &mut blockchain,
        keypair.resume_service(
            ToySupervisorService::INSTANCE_ID,
            ResumeService {
                instance_id: v02::MigrationService::INSTANCE_ID,
                artifact: v02::MigrationService.artifact_id(),
                params: vec![],
            },
        ),
    )
    .unwrap();
    assert_eq!(
        events_handle.take(),
        vec![
            RuntimeEvent::BeforeTransactions(Height(3), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::StartResumingService(migration_service.clone(), vec![]),
            RuntimeEvent::AfterTransactions(Height(3), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::CommitService(
                Height(4),
                v02::MigrationService.default_instance().instance_spec,
                InstanceStatus::Active,
            ),
            RuntimeEvent::AfterCommit(Height(4)),
        ]
    );

    // Check instance state after migration and resume.
    let instance_state = blockchain
        .snapshot()
        .for_dispatcher()
        .get_instance(v02::MigrationService::INSTANCE_ID)
        .unwrap();

    assert_eq!(instance_state.spec, migration_service);
    assert_eq!(instance_state.status, Some(InstanceStatus::Active));
    assert_eq!(
        instance_state.data_version(),
        &migration_service.artifact.version
    );
}

#[test]
fn migration_errors() {
    let (mut blockchain, events_handle) = create_runtime();
    let keypair = blockchain.as_ref().service_keypair().clone();

    // Stop running service instance.
    execute_transaction(
        &mut blockchain,
        keypair.stop_service(
            ToySupervisorService::INSTANCE_ID,
            StopService {
                instance_id: v01::MigrationService::INSTANCE_ID,
            },
        ),
    )
    .unwrap();
    // We not interested in events in this case.
    drop(events_handle.take());

    // Attempt to upgrade service to an unrelated artifact.
    let err = execute_transaction(
        &mut blockchain,
        keypair.migrate_service(
            ToySupervisorService::INSTANCE_ID,
            MigrateService {
                instance_name: v01::MigrationService::INSTANCE_NAME.to_owned(),
                artifact: ToySupervisorService.artifact_id(),
            },
        ),
    )
    .expect_err("Upgrade to an unrelated artifact should fail.");
    assert_eq!(err, ErrorMatch::from_fail(&CoreError::CannotUpgradeService));
    drop(events_handle.take());

    // Attempt to migrate to the same version.
    let err = execute_transaction(
        &mut blockchain,
        keypair.migrate_service(
            ToySupervisorService::INSTANCE_ID,
            MigrateService {
                instance_name: v01::MigrationService::INSTANCE_NAME.to_owned(),
                artifact: v01::MigrationService.artifact_id(),
            },
        ),
    )
    .expect_err("Upgrade to the same version should fail.");
    assert_eq!(err, ErrorMatch::from_fail(&CoreError::CannotUpgradeService));
    drop(events_handle.take());

    // Attempt to migrate unknown service.
    let err = execute_transaction(
        &mut blockchain,
        keypair.migrate_service(
            ToySupervisorService::INSTANCE_ID,
            MigrateService {
                instance_name: "unknown".to_string(),
                artifact: v02::MigrationService.artifact_id(),
            },
        ),
    )
    .expect_err("Upgrade of unknown service should fail.");
    assert_eq!(err, ErrorMatch::from_fail(&CoreError::IncorrectInstanceId));
    drop(events_handle.take());

    // Attempt to migrate to unknown artifact.
    let err = execute_transaction(
        &mut blockchain,
        keypair.migrate_service(
            ToySupervisorService::INSTANCE_ID,
            MigrateService {
                instance_name: v01::MigrationService::INSTANCE_NAME.to_owned(),
                artifact: ArtifactId::from_raw_parts(
                    0,
                    "unknown".to_string(),
                    "0.2.0".parse().unwrap(),
                ),
            },
        ),
    )
    .expect_err("Upgrade to unknown artifact should fail.");
    assert_eq!(err, ErrorMatch::from_fail(&CoreError::UnknownArtifactId));
    drop(events_handle.take());

    // Attempt to upgrade to service without migration support.
    let err = execute_transaction(
        &mut blockchain,
        keypair.migrate_service(
            ToySupervisorService::INSTANCE_ID,
            MigrateService {
                instance_name: v01::MigrationService::INSTANCE_NAME.to_owned(),
                artifact: v03::MigrationService.artifact_id(),
            },
        ),
    )
    .expect_err("Upgrade to service without migration support should fail.");
    let expected_err = ExecutionError::from(InitMigrationError::NotSupported).to_match();
    assert_eq!(err, expected_err);
    drop(events_handle.take());

    // Resume stopped service instance.
    execute_transaction(
        &mut blockchain,
        keypair.resume_service(
            ToySupervisorService::INSTANCE_ID,
            ResumeService {
                instance_id: v01::MigrationService::INSTANCE_ID,
                artifact: v01::MigrationService.artifact_id(),
                params: vec![],
            },
        ),
    )
    .unwrap();
    let migration_service = v01::MigrationService.default_instance().instance_spec;
    assert_eq!(
        events_handle.take(),
        vec![
            RuntimeEvent::BeforeTransactions(Height(7), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::StartResumingService(migration_service.clone(), vec![]),
            RuntimeEvent::AfterTransactions(Height(7), ToySupervisorService::INSTANCE_ID),
            RuntimeEvent::CommitService(
                Height(8),
                v01::MigrationService.default_instance().instance_spec,
                InstanceStatus::Active,
            ),
            RuntimeEvent::AfterCommit(Height(8)),
        ]
    );
}

#[test]
fn migration_downgrade() {
    let (mut blockchain, events_handle) = create_runtime_custom(
        v03::MigrationService,
        vec![v02::MigrationService],
        vec![v01::MigrationService],
    );
    let keypair = blockchain.as_ref().service_keypair().clone();

    // Stop running service instance.
    execute_transaction(
        &mut blockchain,
        keypair.stop_service(
            ToySupervisorService::INSTANCE_ID,
            StopService {
                instance_id: v03::MigrationService::INSTANCE_ID,
            },
        ),
    )
    .unwrap();
    // We not interested in events in this case.
    drop(events_handle.take());

    // Attempt to downgrade service.
    let err = execute_transaction(
        &mut blockchain,
        keypair.migrate_service(
            ToySupervisorService::INSTANCE_ID,
            MigrateService {
                instance_name: v03::MigrationService::INSTANCE_NAME.to_owned(),
                artifact: v02::MigrationService.artifact_id(),
            },
        ),
    )
    .expect_err("Downgrade should fail.");
    assert_eq!(err, ErrorMatch::from_fail(&CoreError::CannotUpgradeService));
}
