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
    merkledb::{BinaryValue, Snapshot},
    runtime::{CallContext, Caller, DispatcherError, MethodId, SUPERVISOR_SERVICE_ID},
};

use super::{ExecutionError, Interface, TransactionContext};

/// Identifier of the [`Initialize::initialize`] method.
///
/// [`Initialize::initialize`]: trait.Initialize.html#tymethod.initialize
pub const INITIALIZE_METHOD_ID: MethodId = 0;
/// Fully qualified name of the `Initialize` interface.
pub const INITIALIZE_INTERFACE_NAME: &str = "Initialize";

// TODO This interface looks a little bit more specific than the other interfaces like
// Configure and it is possible redundant. [ECR-3222]

/// This trait describes a service interface to pass initial configuration parameters into
/// the started service instance.
///
/// This interface is optional, therefore if initialize was called on a service
/// that does not implement this interface, and then if the configuration parameters are empty,
/// dispatcher assumes that the initialization was successful.
pub trait Initialize {
    /// The specific type of parameters passed during the service instance initialization.
    type Params: BinaryValue;

    /// Initialize a service instance with the given parameters.
    ///
    /// The configuration parameters passed to the method are discarded immediately.
    /// So the service instance should save them by itself if it is important for
    /// the service business logic.
    ///
    /// This method is called after creating a new service instance by the [`start_service`]
    /// invocation. In the case of an error occurring during this action, the dispatcher will
    /// invoke [`stop_service`].
    ///
    /// # Execution policy
    ///
    /// This method can only be called on behalf of the [`Blockchain`].
    ///
    /// [`start_service`]: ../../trait.Runtime.html#tymethod.start_service
    /// [`stop_service`]: ../../trait.Runtime.html#tymethod.stop_service
    /// [`Blockchain`]: ../../enum.Caller.html#variant.Blockchain
    fn initialize(
        &self,
        context: TransactionContext,
        params: Self::Params,
    ) -> Result<(), ExecutionError>;
}

impl<T: BinaryValue> Interface for dyn Initialize<Params = T> {
    const INTERFACE_NAME: &'static str = INITIALIZE_INTERFACE_NAME;

    fn dispatch(
        &self,
        context: TransactionContext,
        method: MethodId,
        payload: &[u8],
    ) -> Result<(), ExecutionError> {
        if context.caller().as_blockchain().is_none() {
            let msg = "Methods from the `Initialize` interface should only be called on behalf of the `Blockchain`.";
            return Err(DispatcherError::unauthorized_caller(msg));
        }

        match method {
            INITIALIZE_METHOD_ID => self.initialize(
                context,
                T::from_bytes(payload.into()).map_err(DispatcherError::malformed_arguments)?,
            ),
            other => {
                let kind = DispatcherError::NoSuchMethod;
                let message = format!(
                    "Method with ID {} is absent in the 'Initialize' interface of the instance `{}`",
                    other, context.instance.name,
                );
                Err((kind, message)).map_err(From::from)
            }
        }
    }
}

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
    /// on the particular implementation.
    ///
    /// # Execution policy
    ///
    /// This method can only be called on behalf of the supervisor service instance.
    /// In other words, only a method with the specified [identifier] can call this method.
    ///
    /// [identifier]: ../../constant.SUPERVISOR_SERVICE_ID.html
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
    /// This method is called then some external conditions occur and thus this happens
    /// outside of the transaction execution, which means that errors that occur during the
    /// execution of this method may be ignored.
    ///
    /// # Execution policy
    ///
    /// This method can only be called on behalf of the supervisor service instance.
    /// In other words, only a method with the specified [identifier] can call this method.
    ///
    /// [identifier]: ../../constant.SUPERVISOR_SERVICE_ID.html
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

#[derive(Debug)]
pub struct ConfigureCall<'a>(CallContext<'a>);

impl<'a> From<CallContext<'a>> for ConfigureCall<'a> {
    fn from(context: CallContext<'a>) -> Self {
        Self(context)
    }
}

impl<'a> ConfigureCall<'a> {
    pub fn verify_config(&self, params: impl BinaryValue) -> Result<(), ExecutionError> {
        self.0
            .call(CONFIGURE_INTERFACE_NAME, VERIFY_CONFIG_METHOD_ID, params)
    }

    pub fn apply_config(&self, params: impl BinaryValue) -> Result<(), ExecutionError> {
        self.0
            .call(CONFIGURE_INTERFACE_NAME, APPLY_CONFIG_METHOD_ID, params)
    }
}

pub fn caller_is_supervisor(caller: &Caller, _: &dyn Snapshot) -> Result<(), ExecutionError> {
    caller
        .as_service()
        .and_then(|instance_id| {
            if instance_id == SUPERVISOR_SERVICE_ID {
                Some(())
            } else {
                None
            }
        })
        .ok_or_else(|| {
            DispatcherError::unauthorized_caller(
                "Only the supervisor service is allowed to call this method.",
            )
        })
}
