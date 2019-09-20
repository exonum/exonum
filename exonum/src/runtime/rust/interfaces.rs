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

//! Important interservice communication interfaces.

use crate::{
    merkledb::BinaryValue,
    runtime::{CallContext, Caller, DispatcherError, MethodId, SUPERVISOR_INSTANCE_ID},
};

use super::{ExecutionError, Interface, TransactionContext};

/// Fully qualified name of the ['Configure`] interface.
///
/// ['Configure`]: trait.Configure.html
pub const CONFIGURE_INTERFACE_NAME: &str = "Configure";

/// Identifier of the [`Configure::verify_config`] method.
///
/// [`Configure::verify_config`]: trait.Configure.html#tymethod.verify_config
pub const VERIFY_CONFIG_METHOD_ID: MethodId = 0;
/// Identifier of the [`Configure::apply_config`] method.
///
/// [`Configure::apply_config`]: trait.Configure.html#tymethod.apply_config
pub const APPLY_CONFIG_METHOD_ID: MethodId = 1;

/// Describes a procedure for updating the configuration of a service instance.
pub trait Configure {
    /// The specific type of parameters passed during the service instance configuration.    
    type Params: BinaryValue;
    /// Verify a new configuration parameters before before their actual application.
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
    /// In other words, only a method with the specified [identifier] can call this method.
    ///
    /// [identifier]: ../../constant.SUPERVISOR_INSTANCE_ID.html
    fn verify_config(
        &self,
        context: TransactionContext,
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
    /// In other words, only a method with the specified [identifier] can call this method.
    ///
    /// [identifier]: ../../constant.SUPERVISOR_INSTANCE_ID.html
    fn apply_config(
        &self,
        context: TransactionContext,
        params: Self::Params,
    ) -> Result<(), ExecutionError>;
}

impl<T: BinaryValue> Interface for dyn Configure<Params = T> {
    const INTERFACE_NAME: &'static str = CONFIGURE_INTERFACE_NAME;

    fn dispatch(
        &self,
        context: TransactionContext,
        method: MethodId,
        payload: &[u8],
    ) -> Result<(), ExecutionError> {
        match method {
            VERIFY_CONFIG_METHOD_ID => self.verify_config(
                context,
                T::from_bytes(payload.into()).map_err(DispatcherError::malformed_arguments)?,
            ),

            APPLY_CONFIG_METHOD_ID => self.apply_config(
                context,
                T::from_bytes(payload.into()).map_err(DispatcherError::malformed_arguments)?,
            ),

            other => {
                let kind = DispatcherError::NoSuchMethod;
                let message = format!(
                    "Method with ID {} is absent in the 'Configure' interface of the instance `{}`",
                    other, context.instance.name,
                );
                Err((kind, message)).map_err(From::from)
            }
        }
    }
}

/// A helper struct for invoking the [`Configure`] interface methods on the specified service instance.
///
/// [`Configure`]: trait.Configure.html
#[derive(Debug)]
pub struct ConfigureCall<'a>(CallContext<'a>);

impl<'a> From<CallContext<'a>> for ConfigureCall<'a> {
    fn from(context: CallContext<'a>) -> Self {
        Self(context)
    }
}

impl<'a> ConfigureCall<'a> {
    /// Invoke the corresponding [method](trait.Configure.html#tymethod.verify_config)
    /// of the interface.
    pub fn verify_config(&self, params: impl BinaryValue) -> Result<(), ExecutionError> {
        self.0
            .call(CONFIGURE_INTERFACE_NAME, VERIFY_CONFIG_METHOD_ID, params)
    }

    /// Invoke the corresponding [method](trait.Configure.html#tymethod.apply_config)
    /// of the interface.
    pub fn apply_config(&self, params: impl BinaryValue) -> Result<(), ExecutionError> {
        self.0
            .call(CONFIGURE_INTERFACE_NAME, APPLY_CONFIG_METHOD_ID, params)
    }
}

/// Verify that the caller of this method is supervisor service.
pub fn verify_caller_is_supervisor(caller: &Caller) -> Option<()> {
    caller.as_service().and_then(|instance_id| {
        if instance_id == SUPERVISOR_INSTANCE_ID {
            Some(())
        } else {
            None
        }
    })
}
