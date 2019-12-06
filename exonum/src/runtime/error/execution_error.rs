//! `serde` methods for `ExecutionError`.

use serde::{de::Error, Deserialize, Deserializer, Serialize, Serializer};

use super::{execution_result::ExecutionStatus, ExecutionError};

pub fn serialize<S>(inner: &ExecutionError, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    ExecutionStatus::from(Err(inner)).serialize(serializer)
}

pub fn deserialize<'a, D>(deserializer: D) -> Result<ExecutionError, D::Error>
where
    D: Deserializer<'a>,
{
    ExecutionStatus::deserialize(deserializer).and_then(|status| {
        status
            .into_result()
            .and_then(|res| match res {
                Err(err) => Ok(err),
                Ok(()) => Err("Not an error"),
            })
            .map_err(D::Error::custom)
    })
}
