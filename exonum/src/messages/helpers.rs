use encoding::{Error, Field};
use storage::StorageValue;

///Transaction binary form, can be converted
pub trait BinaryForm: Sized {
    /// Converts transaction into serialized form.
    fn serialize(self) -> Result<Vec<u8>, Error>;

    /// Converts serialized byte array into transaction.
    fn deserialize(buffer: &[u8]) -> Result<Self, Error>;
}

impl<T> BinaryForm for T
where
    T: StorageValue + for<'a> Field<'a>,
{
    fn serialize(self) -> Result<Vec<u8>, Error> {
        Ok(self.into_bytes())
    }

    fn deserialize(buffer: &[u8]) -> Result<Self, Error> {
        <Self as Field>::check(
            buffer,
            0.into(),
            <Self as Field>::field_size().into(),
            <Self as Field>::field_size().into(),
        ).map(|_| StorageValue::from_bytes(buffer.into()))
    }
}
