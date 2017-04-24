/* use serde::{Serialize, Serializer};
use serde::de::{self, Visitor, Deserialize, Deserializer};

use std::fmt;
use std::time::{SystemTime, Duration, UNIX_EPOCH};

pub struct U64(pub u64);
pub struct I64(pub i64);

pub struct SystemTimeSerdeHelper(pub SystemTime);

impl Serialize for U64 {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        ser.serialize_str(&self.0.to_string())
    }
}


impl Deserialize for U64 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer
    {
        struct U64Visitor;

        impl Visitor for U64Visitor {
            type Value = U64;

            fn expecting(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
                write!(fmt, "expecting u64 in str.")
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
                where E: de::Error
            {
                s.parse().map(U64).map_err(|_| {
                    de::Error::custom("Not a valid string representation of u64 integer")
                })
            }
        }
        deserializer.deserialize_str(U64Visitor)
    }
}


impl Serialize for I64 {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        ser.serialize_str(&self.0.to_string())
    }
}


impl Deserialize for I64 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer
    {
        struct I64Visitor;

        impl Visitor for I64Visitor {
            type Value = I64;

            fn expecting(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
                write!(fmt, "expecting i64 in str.")
            }

            fn visit_str<E>( self, s: &str) -> Result<Self::Value, E>
                where E: de::Error
            {
                s.parse().map(I64).map_err(|_| {
                    de::Error::custom("Not a valid string representation of u64 integer")
                })
            }
        }
        deserializer.deserialize_str(I64Visitor)
    }
}

impl Serialize for SystemTimeSerdeHelper {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        let duration = self.0.duration_since(UNIX_EPOCH).unwrap();
        let helper = DurationSerdeHelper {
            secs: U64(duration.as_secs()),
            nanos: duration.subsec_nanos()
        };
        helper.serialize(ser)
    }
}

impl Deserialize for SystemTimeSerdeHelper {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer
    {
        let helper = <DurationSerdeHelper>::deserialize(deserializer)?;
        let duration = Duration::new(helper.secs.0, helper.nanos);
        Ok(SystemTimeSerdeHelper(UNIX_EPOCH + duration))
    }
}

#[derive(Serialize, Deserialize)]
struct DurationSerdeHelper {
    secs: U64,
    nanos: u32,
}

#[cfg(test)]
mod tests {
	use std::time::SystemTime;
	use serde_json; 
	use super::{U64, I64, SystemTimeSerdeHelper, DurationSerdeHelper};

	#[test]
	fn test_serialize() {
		let var = 1486750447235849000; 
		let str_json = serde_json::to_string(&U64(var)).unwrap(); 
		let var1 = serde_json::from_str::<U64>(&str_json).unwrap().0; 
		assert_eq!(var, var1);
		let var_i = -1486750447235849000; 
		let str_json = serde_json::to_string(&I64(var_i)).unwrap(); 
		let var1_i = serde_json::from_str::<I64>(&str_json).unwrap().0; 
		assert_eq!(var_i, var1_i);
	}

	#[test]
	fn test_timespce_helper_serialize() {
		let time = SystemTime::now();
		let str_json = serde_json::to_string(&SystemTimeSerdeHelper(time)).unwrap();
		let time1 = serde_json::from_str::<SystemTimeSerdeHelper>(&str_json).unwrap().0;
		assert_eq!(time, time1);
	}

    #[test]
    fn test_duration_helper_serialize() {
        let helper = DurationSerdeHelper { secs: U64(10), nanos: 20 };
        let str_json = serde_json::to_string(&helper).unwrap();
        let helper1 = serde_json::from_str::<DurationSerdeHelper>(&str_json).unwrap();
        assert_eq!(helper.secs.0, helper1.secs.0);
        assert_eq!(helper.nanos, helper1.nanos);
    }
}
*/