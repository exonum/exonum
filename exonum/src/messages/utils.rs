use time::Timespec;
use serde::{Serialize, Serializer};
use serde::de::{self, Visitor, Deserialize, Deserializer};


pub struct U64(pub u64);
pub struct I64(pub i64);

pub struct TimespecSerdeHelper(pub Timespec); 

impl Serialize for U64 {
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        ser.serialize_str(&self.0.to_string())
    }
}


impl Deserialize for U64 {
    fn deserialize<D>(deserializer: &mut D) -> Result<Self, D::Error>
        where D: Deserializer
    {
        struct U64Visitor;

        impl Visitor for U64Visitor {
            type Value = U64;

            fn visit_str<E>(&mut self, s: &str) -> Result<Self::Value, E>
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
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        ser.serialize_str(&self.0.to_string())
    }
}


impl Deserialize for I64 {
    fn deserialize<D>(deserializer: &mut D) -> Result<Self, D::Error>
        where D: Deserializer
    {
        struct I64Visitor;

        impl Visitor for I64Visitor {
            type Value = I64;

            fn visit_str<E>(&mut self, s: &str) -> Result<Self::Value, E>
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

impl Serialize for TimespecSerdeHelper {
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        let nsec = (self.0.sec as u64) * 1_000_000_000 + self.0.nsec as u64;
        U64(nsec).serialize(ser)
    }
}

impl Deserialize for TimespecSerdeHelper {
    fn deserialize<D>(deserializer: &mut D) -> Result<Self, D::Error>
        where D: Deserializer
    {
    	let nsec = <U64>::deserialize(deserializer)?; 

        let spec = Timespec {
            sec: (nsec.0 / 1_000_000_000) as i64,
            nsec: (nsec.0 % 1_000_000_000) as i32,
        }; 
        Ok(TimespecSerdeHelper(spec))
    }
}

#[cfg(test)]
mod tests {
	use time::{get_time};
	use serde_json; 
	use super::{U64, I64, TimespecSerdeHelper}; 

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
		let time = get_time(); 
		let str_json = serde_json::to_string(&TimespecSerdeHelper(time)).unwrap(); 
		let time1 = serde_json::from_str::<TimespecSerdeHelper>(&str_json).unwrap().0; 
		assert_eq!(time, time1);
	}
}
