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
pub trait Interface {
    /// Fully qualified name of this interface.
    const INTERFACE_NAME: &'static str;

    /// Invokes the specified method handler of the service instance.
    fn dispatch(
        &self,
        cx: CallContext<'_>,
        method: MethodId,
        payload: &[u8],
    ) -> Result<(), ExecutionError>;
}

/// Generic / low-level stub implementation which is defined for any method in any interface.
pub trait CallStub {
    /// Type of values output by the stub.
    type Output;
    /// Calls a stub method.
    fn call_stub(&mut self, method: MethodDescriptor<'_>, args: Vec<u8>) -> Self::Output;
}

/// Stub that creates unsigned transactions.
#[derive(Debug, Clone, Copy)]
pub struct TxStub(pub InstanceId);

impl TxStub {
    /// Converts this stub into a signer that uses the provided pair of keys.
    pub fn into_signer(self, public_key: PublicKey, secret_key: SecretKey) -> Signer {
        Signer {
            instance_id: self.0,
            public_key,
            secret_key,
        }
    }

    /// Creates a signer with a random keypair.
    pub fn with_random_keypair(self) -> Signer {
        let (public_key, secret_key) = exonum_crypto::gen_keypair();
        self.into_signer(public_key, secret_key)
    }
}

impl CallStub for TxStub {
    type Output = AnyTx;

    fn call_stub(&mut self, method: MethodDescriptor<'_>, args: Vec<u8>) -> Self::Output {
        if !method.interface_name.is_empty() {
            panic!("Creating transactions with non-default interface is not yet supported");
        }

        let call_info = CallInfo::new(self.0, method.id);
        AnyTx {
            call_info,
            arguments: args,
        }
    }
}

/// Transaction signer, i.e., a stub that creates signed transactions.
///
/// A signer can be obtained from a [`TxStub`].
///
/// [`TxStub`]: struct.TxStub.html
#[derive(Debug, Clone)]
pub struct Signer {
    instance_id: InstanceId,
    public_key: PublicKey,
    secret_key: SecretKey,
}

impl Signer {
    /// Returns the public key used by the signer.
    pub fn public_key(&self) -> &PublicKey {
        &self.public_key
    }
}

impl CallStub for Signer {
    type Output = Verified<AnyTx>;

    fn call_stub(&mut self, method: MethodDescriptor<'_>, args: Vec<u8>) -> Self::Output {
        let tx = TxStub(self.instance_id).call_stub(method, args);
        Verified::from_value(tx, self.public_key, &self.secret_key)
    }
}
