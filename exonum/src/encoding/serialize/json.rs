// Copyright 2018 The Exonum Team
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

/// trait `ExonumSerializeJson` implemented for all field that allows serializing in
/// json format.
///

// TODO refer to difference between json serialization and exonum_json (ECR-156).
// TODO implement Field for float (ECR-153).
// should be moved into storage (ECR-156).

// TODO: should we implement serialize for: `SecretKey`, `Seed` (ECR-156)?

/// Reexport of `serde` specific traits, this reexports
/// provide compatibility layer with important `serde_json` version.
pub mod reexport {
    pub use serde_json::{from_str, from_value, to_string, to_value, Error, Value};
    pub use serde_json::map::Map;
}

#[cfg(test)]
mod tests {
    #![allow(unsafe_code)]

    use serde_json::to_value;
    use chrono::Duration;

    use super::*;
    use encoding::{CheckedOffset, Field, Offset};

    #[test]
    fn exonum_json_for_duration_round_trip() {
        let durations = [
            Duration::zero(),
            Duration::max_value(),
            Duration::min_value(),
            Duration::nanoseconds(999_999_999),
            Duration::nanoseconds(-999_999_999),
            Duration::seconds(42) + Duration::nanoseconds(15),
            Duration::seconds(-42) + Duration::nanoseconds(-15),
        ];

        // Variables for serialization/deserialization
        let mut buffer = vec![0; Duration::field_size() as usize];
        let from: Offset = 0;
        let to: Offset = Duration::field_size();
        let checked_from = CheckedOffset::new(from);
        let checked_to = CheckedOffset::new(to);

        for duration in durations.iter() {
//            let serialized = to_value(duration)
//                .expect("Can't serialize duration");
//
//            Duration::from_value(serialized, &mut buffer, from, to)
//                .expect("Can't deserialize duration");
//
//            Duration::check(&buffer, checked_from, checked_to, checked_to)
//                .expect("Incorrect result of deserialization");
//
//            let result_duration;
//
//            unsafe {
//                result_duration = Duration::read(&buffer, from, to);
//            }
//
//            assert_eq!(*duration, result_duration);
        }
    }

}
