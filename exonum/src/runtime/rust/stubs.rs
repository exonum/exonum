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

//! Stubs allowing to call interfaces of Exonum services on types satisfying certain requirements.
//!
//! See the module-level docs for the Rust runtime for an explanation how to use stubs,
//! and the `explanation` module below for an explanation how stubs work.

use exonum_crypto::{PublicKey, SecretKey};

use crate::{
    messages::Verified,
    runtime::{rust::CallContext, AnyTx, CallInfo, ExecutionError, InstanceId, MethodId},
};

/// Descriptor of a method declared as a part of the service interface.
#[derive(Debug, Clone, Copy)]
pub struct MethodDescriptor<'a> {
    /// Name of the interface.
    pub interface_name: &'a str,
    /// Name of the method.
    pub name: &'a str,
    /// Numerical ID of the method.
    pub id: MethodId,
}

impl<'a> MethodDescriptor<'a> {
    /// Creates the descriptor based on provided properties.
    pub const fn new(interface_name: &'a str, name: &'a str, id: MethodId) -> Self {
        Self {
            interface_name,
            name,
            id,
        }
    }
}

/// A service interface specification.
pub trait Interface<'a> {
    /// Fully qualified name of this interface.
    const INTERFACE_NAME: &'static str;

    /// Invokes the specified method handler of the service instance.
    fn dispatch(
        &self,
        cx: CallContext<'a>,
        method: MethodId,
        payload: &[u8],
    ) -> Result<(), ExecutionError>;
}

/// Generic / low-level stub implementation which is defined for any method in any interface.
pub trait GenericCall<Ctx> {
    /// Type of values output by the stub.
    type Output;
    /// Calls a stub method.
    fn generic_call(
        &self,
        context: Ctx,
        method: MethodDescriptor<'_>,
        args: Vec<u8>,
    ) -> Self::Output;
}

/// Generic / low-level stub implementation which is defined for any method in any interface.
/// Differs from `GenericCall` by taking `self` by the mutable reference.
///
/// Implementors should implement `GenericCallMut` only when using `GenericCall` is impossible.
pub trait GenericCallMut<Ctx> {
    /// Type of values output by the stub.
    type Output;
    /// Calls a stub method.
    fn generic_call_mut(
        &mut self,
        context: Ctx,
        method: MethodDescriptor<'_>,
        args: Vec<u8>,
    ) -> Self::Output;
}

/// Stub that creates unsigned transactions.
#[derive(Debug, Clone, Copy)]
pub struct TxStub;

impl GenericCall<InstanceId> for TxStub {
    type Output = AnyTx;

    fn generic_call(
        &self,
        instance_id: InstanceId,
        method: MethodDescriptor<'_>,
        args: Vec<u8>,
    ) -> Self::Output {
        if !method.interface_name.is_empty() {
            panic!("Creating transactions with non-default interface is not yet supported");
        }

        let call_info = CallInfo::new(instance_id, method.id);
        AnyTx {
            call_info,
            arguments: args,
        }
    }
}

impl GenericCall<InstanceId> for (PublicKey, SecretKey) {
    type Output = Verified<AnyTx>;

    fn generic_call(
        &self,
        instance_id: InstanceId,
        method: MethodDescriptor<'_>,
        args: Vec<u8>,
    ) -> Self::Output {
        let tx = TxStub.generic_call(instance_id, method, args);
        Verified::from_value(tx, self.0, &self.1)
    }
}

#[cfg(test)]
mod explanation {
    use super::*;
    use exonum_crypto::gen_keypair;
    use exonum_merkledb::BinaryValue;

    // Suppose we have the following trait describing user service.
    trait Token<Ctx> {
        type Output;

        fn create_wallet(&self, context: Ctx, wallet: CreateWallet) -> Self::Output;
        fn transfer(&self, context: Ctx, transfer: Transfer) -> Self::Output;
    }

    // The `Ctx` type param allows to provide additional information to the implementing type.
    // For example, many stubs require to know the instance ID to which the call is addressed.
    // For these stubs `Ctx == InstanceId` may make sense. In other cases, the context
    // may be void `()`.

    // We don't quite care about types here, so we define them as:
    type CreateWallet = String;
    type Transfer = u64;
    // In general, we accept any type implementing the `BinaryValue` trait.

    // Our goal is to provide an implementation of this user-defined trait for some generic
    // types, e.g., a keypair (which would generate signed transactions when called), or
    // `CallContext` (which would call another service on the same blockchain).

    // In order to accomplish this, we notice that for all possible service traits,
    // there exists a uniform conversion of arguments: the argument (i.e.,
    // `wallet` for `create_wallet`, `transfer` for `transfer`) can always be converted to
    // a `Vec<u8>` since it implements the `BinaryValue` trait. Moreover, this conversion
    // is performed by the stub types anyway (e.g., the keypair needs to get the binary serialization
    // of the message in order to create a signature on it).

    // Similarly, the information about the method itself is also uniform; it consists of
    // the method ID and name. This info is encapsulated in the `MethodDescriptor` type
    // in the parent module.

    // The existence of uniform conversions gives us an approach to the solution. We need
    // to define a more generic trait (`GenericCall` / `GenericCallMut`), which would then
    // be implemented for any user-defined service interface like this:
    impl<T, Ctx> Token<Ctx> for T
    where
        T: GenericCall<Ctx>,
    {
        type Output = <T as GenericCall<Ctx>>::Output;

        fn create_wallet(&self, context: Ctx, wallet: CreateWallet) -> Self::Output {
            const DESCRIPTOR: MethodDescriptor<'static> = MethodDescriptor {
                interface_name: "",
                name: "create_wallet",
                id: 0,
            };
            self.generic_call(context, DESCRIPTOR, wallet.into_bytes())
        }

        fn transfer(&self, context: Ctx, transfer: Transfer) -> Self::Output {
            const DESCRIPTOR: MethodDescriptor<'static> = MethodDescriptor {
                interface_name: "",
                name: "transfer",
                id: 1,
            };
            self.generic_call(context, DESCRIPTOR, transfer.into_bytes())
        }
    }

    // This is exactly the kind of code generated by the `#[exonum_interface]` macro.

    // ...And that's it. As long as the interface trait is in scope, we can use its methods
    // on any type implementing `GenericCall`:
    #[test]
    fn standard_stubs_work() {
        const SERVICE_ID: InstanceId = 100;

        let keypair = gen_keypair();
        let tx: Verified<AnyTx> = keypair.create_wallet(SERVICE_ID, CreateWallet::default());
        assert_eq!(tx.payload().call_info.method_id, 0);
        let other_tx = keypair.transfer(SERVICE_ID, Transfer::default());
        assert_eq!(other_tx.payload().call_info.method_id, 1);
    }

    // It's also possible to define new stubs (not necessarily in this crate). For example,
    // this stub outputs the size of the payload.
    struct PayloadSize;
    impl GenericCall<()> for PayloadSize {
        type Output = usize;

        fn generic_call(
            &self,
            _context: (),
            _method: MethodDescriptor<'_>,
            args: Vec<u8>,
        ) -> Self::Output {
            args.len()
        }
    }

    #[test]
    fn custom_stub() {
        let len = PayloadSize.create_wallet((), "Alice".into());
        assert_eq!(len, 5);
        let len = PayloadSize.transfer((), 42);
        assert_eq!(len, 8);
    }
}
