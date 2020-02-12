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

//! Configuration interface used by the supervisor to change service configuration.

use exonum::runtime::{CommonError, ExecutionContext, ExecutionError, InstanceId, MethodId};
use exonum_merkledb::BinaryValue;
use exonum_rust_runtime::{GenericCallMut, Interface, MethodDescriptor};

/// Fully qualified name of the ['Configure`] interface.
///
/// ['Configure`]: trait.Configure.html
pub const CONFIGURE_INTERFACE_NAME: &str = "exonum.Configure";

/// Identifier of the [`Configure::verify_config`] method.
///
/// [`Configure::verify_config`]: trait.Configure.html#tymethod.verify_config
const VERIFY_CONFIG_METHOD_ID: MethodId = 0;

/// Identifier of the [`Configure::apply_config`] method.
///
/// [`Configure::apply_config`]: trait.Configure.html#tymethod.apply_config
const APPLY_CONFIG_METHOD_ID: MethodId = 1;

/// Describes a procedure for updating the configuration of a service instance.
pub trait Configure {
    /// The specific type of parameters passed during the service instance configuration.
    type Params: BinaryValue;

    /// Verify a new configuration parameters before their actual application.
    ///
    /// This method is called by the new configuration change proposal. If the proposed
    /// parameters do not fit for this service instance, it should return a corresponding
    /// error to discard this proposal. Thus only a configuration change proposal in which all
    /// changes are correct can be applied later.
    ///
    /// The proposal approval process details, and even the configuration proposal format, depends
    /// on the particular runtime implementation.
    ///
    /// # Execution policy
    ///
    /// At the moment, this method can only be called on behalf of the supervisor service instance.
    /// In other words, only a method with numeric ID 0 can call this method.
    fn verify_config(
        &self,
        context: ExecutionContext<'_>,
        params: Self::Params,
    ) -> Result<(), ExecutionError>;

    /// Update service configuration with the given parameters.
    ///
    /// The configuration parameters passed to the method are discarded immediately.
    /// So the service instance should save them by itself if it is important for
    /// the service business logic.
    ///
    /// This method can be triggered at any time and does not follow the general transaction
    /// execution workflow, so the errors returned might be ignored.
    ///
    /// # Execution policy
    ///
    /// At the moment, this method can only be called on behalf of the supervisor service instance.
    /// In other words, only a method with numeric ID 0 can call this method.
    fn apply_config(
        &self,
        context: ExecutionContext<'_>,
        params: Self::Params,
    ) -> Result<(), ExecutionError>;
}

impl<'a, T: BinaryValue> Interface<'a> for dyn Configure<Params = T> {
    const INTERFACE_NAME: &'static str = CONFIGURE_INTERFACE_NAME;

    fn dispatch(
        &self,
        context: ExecutionContext<'a>,
        method: MethodId,
        payload: &[u8],
    ) -> Result<(), ExecutionError> {
        match method {
            VERIFY_CONFIG_METHOD_ID => {
                let params =
                    T::from_bytes(payload.into()).map_err(CommonError::malformed_arguments)?;
                self.verify_config(context, params)
            }

            APPLY_CONFIG_METHOD_ID => {
                let params =
                    T::from_bytes(payload.into()).map_err(CommonError::malformed_arguments)?;
                self.apply_config(context, params)
            }

            _ => Err(CommonError::NoSuchMethod.into()),
        }
    }
}

// Makeshift replacement for generic stubbing, which is made difficult by the existence
// of the type param.
pub(crate) trait ConfigureMut<Ctx> {
    type Output;

    fn verify_config(&mut self, context: Ctx, params: Vec<u8>) -> Self::Output;
    fn apply_config(&mut self, context: Ctx, params: Vec<u8>) -> Self::Output;
}

impl ConfigureMut<InstanceId> for ExecutionContext<'_> {
    type Output = Result<(), ExecutionError>;

    fn verify_config(&mut self, instance_id: InstanceId, params: Vec<u8>) -> Self::Output {
        const METHOD_DESCRIPTOR: MethodDescriptor<'static> =
            MethodDescriptor::new(CONFIGURE_INTERFACE_NAME, 0);
        self.generic_call_mut(instance_id, METHOD_DESCRIPTOR, params)
    }

    fn apply_config(&mut self, instance_id: InstanceId, params: Vec<u8>) -> Self::Output {
        const METHOD_DESCRIPTOR: MethodDescriptor<'static> =
            MethodDescriptor::new(CONFIGURE_INTERFACE_NAME, 1);
        self.generic_call_mut(instance_id, METHOD_DESCRIPTOR, params)
    }
}
