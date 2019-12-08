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
