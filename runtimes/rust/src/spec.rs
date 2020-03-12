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

//! Specifications of Rust and non-Rust artifacts for use in deployment.
//!
//! Two consumers of the specifications produced by this module are the [testkit]
//! and the node builder from the [`exonum-cli` crate].
//!
//! [testkit]: https://crates.io/crates/exonum-testkit
//! [`exonum-cli` crate]: https://crates.io/crates/exonum-cli

use exonum::{
    blockchain::config::{GenesisConfigBuilder, InstanceInitParams},
    merkledb::BinaryValue,
    runtime::{migrations::MigrateData, ArtifactId, InstanceId, RuntimeIdentifier},
};

use std::{marker::PhantomData, mem};

use self::sealed::Sealed;
use crate::{DefaultInstance, RustRuntimeBuilder, ServiceFactory};

mod sealed {
    pub trait Sealed {}
}

/// Denotes a data type that can be used as an argument to `NodeBuilder::with()`.
///
/// This is a sealed trait: it is not meant to be implemented by the external data types.
pub trait Deploy: Sealed {
    /// Modifies the genesis config and Rust runtime builders to deploy self.
    fn deploy(self, genesis: &mut GenesisConfigBuilder, rt: &mut RustRuntimeBuilder);
}

/// Marker type for artifact deployment without migration support.
#[derive(Debug)]
pub struct Simple(());

/// Marker type for artifact deployment with migration support.
#[derive(Debug)]
pub struct Migrating(());

/// Deploy specification for a Rust artifact. The spec can include zero or more instantiated services.
///
/// # Examples
///
/// ```
/// # use exonum_derive::*;
/// # use exonum::runtime::InstanceId;
/// # use exonum_rust_runtime::{spec::Spec, DefaultInstance, Service, ServiceFactory};
/// #[derive(Debug, ServiceDispatcher, ServiceFactory)]
/// #[service_factory(artifact_name = "my-service")]
/// pub struct MyService;
/// impl Service for MyService {}
///
/// impl DefaultInstance for MyService {
///     const INSTANCE_ID: InstanceId = 100;
///     const INSTANCE_NAME: &'static str = "my-service";
/// }
///
/// let spec = Spec::new(MyService)
///     .with_default_instance()
///     .with_instance(200, "other-service", 42_u64);
/// // Deploy `spec` somewhere...
/// ```
#[derive(Debug)]
pub struct Spec<T, Kind> {
    service: T,
    instances: Vec<InstanceInitParams>,
    default_instance: Option<InstanceInitParams>,
    _kind: PhantomData<Kind>,
}

impl<T: ServiceFactory, Kind> Spec<T, Kind> {
    /// Adds a new built-in service instance to instantiate at the genesis block.
    pub fn with_instance(
        mut self,
        id: InstanceId,
        name: impl Into<String>,
        constructor: impl BinaryValue,
    ) -> Self {
        self.instances.push(InstanceInitParams::new(
            id,
            name,
            self.service.artifact_id(),
            constructor,
        ));
        self
    }
}

impl<T: DefaultInstance, Kind> Spec<T, Kind> {
    /// Adds a built-in service instance with the default identifiers
    /// to instantiate at the genesis block.
    ///
    /// Calling this method several times still results in a *single* default instance added
    /// to the blockchain upon creation.
    pub fn with_default_instance(mut self) -> Self {
        self.default_instance = Some(self.service.default_instance());
        self
    }
}

impl<T: ServiceFactory> Spec<T, Simple> {
    /// Creates a spec with no support of data migrations.
    pub fn new(service: T) -> Self {
        Self {
            service,
            instances: vec![],
            default_instance: None,
            _kind: PhantomData,
        }
    }
}

impl<T: ServiceFactory> Spec<T, Migrating> {
    /// Creates a spec with support of data migrations.
    pub fn migrating(service: T) -> Self {
        Self {
            service,
            instances: vec![],
            default_instance: None,
            _kind: PhantomData,
        }
    }
}

impl<T: Clone, Kind> Clone for Spec<T, Kind> {
    fn clone(&self) -> Self {
        Self {
            service: self.service.clone(),
            instances: self.instances.clone(),
            default_instance: self.default_instance.clone(),
            _kind: PhantomData,
        }
    }
}

impl<T: ServiceFactory> Sealed for Spec<T, Simple> {}

impl<T: ServiceFactory> Deploy for Spec<T, Simple> {
    fn deploy(self, genesis: &mut GenesisConfigBuilder, rt: &mut RustRuntimeBuilder) {
        let mut new_config = mem::take(genesis).with_artifact(self.service.artifact_id());
        let instances = self.default_instance.into_iter().chain(self.instances);
        for instance in instances {
            new_config = new_config.with_instance(instance);
        }
        *genesis = new_config;

        *rt = mem::take(rt).with_factory(self.service);
    }
}

impl<T: ServiceFactory> Sealed for Spec<T, Migrating> {}

impl<T: ServiceFactory + MigrateData> Deploy for Spec<T, Migrating> {
    fn deploy(self, genesis: &mut GenesisConfigBuilder, rt: &mut RustRuntimeBuilder) {
        let mut new_config = mem::take(genesis).with_artifact(self.service.artifact_id());
        let instances = self.default_instance.into_iter().chain(self.instances);
        for instance in instances {
            new_config = new_config.with_instance(instance);
        }
        *genesis = new_config;

        *rt = mem::take(rt).with_migrating_factory(self.service);
    }
}

/// Deploy specification which just adds a service factory to the Rust runtime without
/// deploying a corresponding artifact. Useful mainly for tests; use `Spec` to deploy the artifact.
#[derive(Debug)]
pub struct JustFactory<T, Kind> {
    service: T,
    _kind: PhantomData<Kind>,
}

impl<T: ServiceFactory> JustFactory<T, Simple> {
    /// Creates a new specification wrapping the provided service factory.
    pub fn new(service: T) -> Self {
        Self {
            service,
            _kind: PhantomData,
        }
    }
}

impl<T: ServiceFactory> Sealed for JustFactory<T, Simple> {}

impl<T: ServiceFactory> Deploy for JustFactory<T, Simple> {
    fn deploy(self, _: &mut GenesisConfigBuilder, rt: &mut RustRuntimeBuilder) {
        *rt = mem::take(rt).with_factory(self.service);
    }
}

impl<T: ServiceFactory + MigrateData> JustFactory<T, Migrating> {
    /// Creates a new specification wrapping the provided service factory with support
    /// of data migrations.
    pub fn migrating(service: T) -> Self {
        Self {
            service,
            _kind: PhantomData,
        }
    }
}

impl<T: ServiceFactory + MigrateData> Sealed for JustFactory<T, Migrating> {}

impl<T: ServiceFactory + MigrateData> Deploy for JustFactory<T, Migrating> {
    fn deploy(self, _: &mut GenesisConfigBuilder, rt: &mut RustRuntimeBuilder) {
        *rt = mem::take(rt).with_migrating_factory(self.service);
    }
}

/// Deploy specification for a non-Rust artifact. The spec can include zero or more
/// instantiated services.
///
/// # Examples
///
/// ```
/// # use exonum::runtime::{versioning::Version, ArtifactId};
/// # use exonum_derive::*;
/// # use exonum_rust_runtime::spec::ForeignSpec;
/// # use serde_derive::*;
/// #
/// # #[derive(Serialize, Deserialize, BinaryValue)]
/// # #[binary_value(codec = "bincode")]
/// # struct ArtifactSpec {}
/// #
/// # fn main() -> anyhow::Result<()> {
/// const MY_RUNTIME_ID: u32 = 42;
/// let artifact = ArtifactId::new(MY_RUNTIME_ID, "test", Version::new(1, 0, 0))?;
/// let spec = ForeignSpec::new(artifact)
///     .with_deploy_spec(ArtifactSpec {
///         // Runtime-specific data necessary to deploy the artifact
///     })
///     .with_instance(100, "test-service", ());
/// // Deploy `spec` somewhere...
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct ForeignSpec {
    artifact: ArtifactId,
    deploy_spec: Option<Vec<u8>>,
    instances: Vec<InstanceInitParams>,
}

impl ForeignSpec {
    /// Creates a spec based on the provided artifact.
    pub fn new(artifact: ArtifactId) -> Self {
        assert_ne!(
            artifact.runtime_id,
            RuntimeIdentifier::Rust as u32,
            "Deploying Rust artifacts with `ForeignSpec` does not make sense; the Rust runtime \
             will not hold the service factory necessary to instantiate corresponding services. \
             Use `Spec` instead"
        );
        Self {
            artifact,
            deploy_spec: None,
            instances: vec![],
        }
    }

    /// Adds a deploy specification to use together with the artifact.
    pub fn with_deploy_spec(mut self, spec: impl BinaryValue) -> Self {
        self.deploy_spec = Some(spec.into_bytes());
        self
    }

    /// Adds a new built-in service instance to instantiate at the genesis block.
    pub fn with_instance(
        mut self,
        id: InstanceId,
        name: impl Into<String>,
        constructor: impl BinaryValue,
    ) -> Self {
        self.instances.push(InstanceInitParams::new(
            id,
            name,
            self.artifact.clone(),
            constructor,
        ));
        self
    }
}

impl Sealed for ForeignSpec {}

impl Deploy for ForeignSpec {
    fn deploy(self, genesis: &mut GenesisConfigBuilder, _: &mut RustRuntimeBuilder) {
        let mut new_config = if let Some(deploy_spec) = self.deploy_spec {
            mem::take(genesis).with_parametric_artifact(self.artifact, deploy_spec)
        } else {
            mem::take(genesis).with_artifact(self.artifact)
        };

        for instance in self.instances {
            new_config = new_config.with_instance(instance);
        }
        *genesis = new_config;
    }
}
