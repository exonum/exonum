pub use crate::proto::schema::*;

use crate::proto::schema::{Hash, PublicKey, Signature};
use crate::{HASH_SIZE, PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH};
use exonum_proto::ProtobufConvert;
use failure::Error;

mod schema;

impl ProtobufConvert for crate::Hash {
    type ProtoStruct = Hash;

    fn to_pb(&self) -> Hash {
        let mut hash = Hash::new();
        hash.set_data(self.as_ref().to_vec());
        hash
    }

    fn from_pb(pb: Hash) -> Result<Self, Error> {
        let data = pb.get_data();
        ensure!(data.len() == HASH_SIZE, "Wrong Hash size");
        crate::Hash::from_slice(data).ok_or_else(|| format_err!("Cannot convert Hash from bytes"))
    }
}

impl ProtobufConvert for crate::PublicKey {
    type ProtoStruct = PublicKey;

    fn to_pb(&self) -> PublicKey {
        let mut key = PublicKey::new();
        key.set_data(self.as_ref().to_vec());
        key
    }

    fn from_pb(pb: PublicKey) -> Result<Self, Error> {
        let data = pb.get_data();
        ensure!(data.len() == PUBLIC_KEY_LENGTH, "Wrong PublicKey size");
        crate::PublicKey::from_slice(data)
            .ok_or_else(|| format_err!("Cannot convert PublicKey from bytes"))
    }
}

impl ProtobufConvert for crate::Signature {
    type ProtoStruct = Signature;

    fn to_pb(&self) -> Signature {
        let mut sign = Signature::new();
        sign.set_data(self.as_ref().to_vec());
        sign
    }

    fn from_pb(pb: Signature) -> Result<Self, Error> {
        let data = pb.get_data();
        ensure!(data.len() == SIGNATURE_LENGTH, "Wrong Signature size");
        crate::Signature::from_slice(data)
            .ok_or_else(|| format_err!("Cannot convert Signature from bytes"))
    }
}
