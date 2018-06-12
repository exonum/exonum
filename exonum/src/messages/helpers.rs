use encoding::Field;
use storage::StorageValue;
use failure::Error;
//TODO: temp trait for internal purposes.
pub trait BinaryForm: Sized {
    fn serialize(self) -> Result<Vec<u8>, Error>;

    fn deserialize(buffer: &[u8]) -> Result<Self, Error>;
}

impl<T> BinaryForm for T
    where T: StorageValue + for<'a> Field<'a>
{
    fn serialize(self) -> Result<Vec<u8>, Error> {
        Ok(self.into_bytes())
    }

    fn deserialize(buffer: &[u8]) -> Result<Self, Error> {
        <Self as Field>::check(
            buffer,
            0.into(),
            <Self as Field>::field_size().into(),
            <Self as Field>::field_size().into())
            .map(|_| StorageValue::from_bytes(buffer.into()))
            .map_err(|e| unimplemented!())
    }
}
