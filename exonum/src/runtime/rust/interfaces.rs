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
    runtime::{DispatcherError, MethodId},
};

use super::{ExecutionError, Interface, TransactionContext};

/// Identifier of the [`Initialize::initialize`] method.
///
/// [`Initialize::initialize`]: trait.Initialize.html#tymethod.initialize
pub const INITIALIZE_METHOD_ID: MethodId = 0;
/// Fully qualified name of the `Initialize` interface.
pub const INITIALIZE_INTERFACE_NAME: &str = "Initialize";

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
    /// invocation. In the case of an error occuring during this action, the dispatcher will
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
