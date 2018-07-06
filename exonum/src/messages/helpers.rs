use super::{Message, RawTransaction, SignedMessage};
use encoding::Error;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

///Transaction binary form, can be converted
pub trait BinaryForm: Sized {
    /// Converts transaction into serialized form.
    fn serialize(self) -> Result<Vec<u8>, Error>;

    /// Converts serialized byte array into transaction.
    fn deserialize(buffer: &[u8]) -> Result<Self, Error>;
}

impl<T> BinaryForm for T
where
    T: Serialize + de::DeserializeOwned,
{
    fn serialize(self) -> Result<Vec<u8>, Error> {
//        let mut buf = vec![0; 8];
//        Field::write(&self, &mut buf, 0, 8);
        Ok(::bincode::config()
            .no_limit()
            .serialize(&self)?)
    }
    #[allow(unsafe_code)]
    fn deserialize(buffer: &[u8]) -> Result<Self, Error> {
//        <Self as Field>::check(
//            buffer,
//            0.into(),
//            <Self as Field>::field_size().into(),
//            <Self as Field>::field_size().into(),
//        ).map(|_| unsafe { <Self as Field>::read(buffer, 0, <Self as Field>::field_size()) })
        Ok(::bincode::config()
            .no_limit()
            .deserialize(buffer)?)
    }
}



/// Serializes `Message<RawTranasction>` as hex value.
pub(crate) struct HexTransaction;
impl HexTransaction {
    pub(crate) fn serialize<S>(
        message: &Message<RawTransaction>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let hex_string = message.to_hex_string();
        <String as Serialize>::serialize(&hex_string, serializer)
    }

    pub(crate) fn deserialize<'a, D>(deserializer: D) -> Result<Message<RawTransaction>, D::Error>
    where
        D: Deserializer<'a>,
    {
        let hex_string = <String as Deserialize>::deserialize(deserializer)?;
        let vec = ::hex::decode(hex_string).map_err(de::Error::custom)?;
        let signed = SignedMessage::verify_buffer(vec).map_err(de::Error::custom)?;
        let msg = signed
            .into_message()
            .map_into::<RawTransaction>()
            .map_err(de::Error::custom)?;
        Ok(msg)
    }
}

/*
/// Serializes `Message<RawTranasction>` as pretty printed debug,
/// along with loseless hex value representation.
pub(crate) struct PrettyMessage {
    message: String,
    debug: BTreeMap<String>
};
impl LoselessMessage {
    fn serialize<S, T>(message: &Message<T>, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
    {
        let hex_string = message.to_hex_string();
        hex_string.serialize(serializer)
    }

    fn deserialize<'a, D, T>(deserializer: D) -> Result<Message<T>, D::Error>
        where
            D: Deserializer<'a>,
    {
        let hex_string = <String as Deserialize>::deserialize(deserializer)?;
        let signed = SignedMessage::verify_buffer(::hex::decode(hex_string)?)?;
        let msg = signed.into_message().map_into::<RawTransaction>()?;
        Ok(msg)
    }
}

*/
