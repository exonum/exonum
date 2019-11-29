# exonum-time

[![Travis Build Status](https://img.shields.io/travis/exonum/exonum/master.svg?label=Linux%20Build)](https://travis-ci.com/exonum/exonum)
[![Docs.rs](https://docs.rs/exonum-time/badge.svg)](https://docs.rs/exonum-time)
[![License: Apache-2.0](https://img.shields.io/github/license/exonum/exonum.svg)](https://github.com/exonum/exonum/blob/master/LICENSE)
![rust 1.36.0+ required](https://img.shields.io/badge/rust-1.36.0+-blue.svg?label=Required%20Rust)

Exonum-time is a time oracle service for [Exonum blockchain framework](https://exonum.com/).
This service allows to determine time,
import it from the external world to the blockchain
and keep its current value in the blockchain.

## Usage

Include `exonum-time` as a dependency in your `Cargo.toml`:

```toml
[dependencies]
exonum = "0.12.0"
exonum-time = "0.12.0"
```

Add the time oracle service to the blockchain in the main project file:

```rust
extern crate exonum;
extern crate exonum_time;

use exonum::helpers::fabric::NodeBuilder;
use exonum_time::TimeServiceFactory;

fn main() {
    exonum::helpers::init_logger().unwrap();
    NodeBuilder::new()
        .with_service(Box::new(TimeServiceFactory))
        .run();
}
```

### Importing the data schema

Typical usage of the service boils down to importing the schema and calling its
`time()` or `validators_time()` methods.

Below is an example of a method for processing a transaction,
which must be executed no later than the specified time
(this time is written in the transaction body in a separate field):

```rust
/// The argument of the `MarkerInterface::mark` method.
#[derive(Serialize, Deserialize, Debug, Clone, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::TxMarker")]
pub struct TxMarker {
    mark: i32,
    time: DateTime<Utc>,
}

/// Marker service transactions interface definition.
#[exonum_interface]
pub trait MarkerTransactions {
    /// Transaction, which must be executed no later than the specified time (field `time`).
    fn mark(&self, context: CallContext<'_>, arg: TxMarker) -> Result<(), ExecutionError>;
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_factory(
    artifact_name = "marker",
    artifact_version = "0.1.0",
    proto_sources = "proto"
)]
#[service_dispatcher(implements("MarkerTransactions"))]
struct MarkerService;

/// Marker service database schema.
#[derive(Debug, FromAccess)]
pub struct MarkerSchema<T: Access> {
    pub marks: ProofMapIndex<T::Base, PublicKey, i32>,
}

impl<T: Access> MarkerSchema<T> {
    /// Returns hashes for stored table.
    fn state_hash(&self) -> Vec<Hash> {
        vec![self.marks.object_hash()]
    }
}

impl MarkerTransactions for MarkerService {
    fn mark(&self, context: CallContext<'_>, arg: TxMarker) -> Result<(), ExecutionError> {
        let author = context
            .caller()
            .author()
            .expect("Wrong `TxMarker` initiator");

        let data = context.data();
        let time_service_data = data
            .for_service(TIME_SERVICE_NAME)
            .expect("No time service data");
        let time = TimeSchema::new(time_service_data).time.get();
        match time {
            Some(current_time) if current_time <= arg.time => {
                let mut schema = MarkerSchema::new(context.service_data());
                schema.marks.put(&author, arg.mark);
            }
            _ => {}
        }
        Ok(())
    }
}

impl Service for MarkerService {
    fn state_hash(&self, data: BlockchainData<&dyn Snapshot>) -> Vec<Hash> {
        MarkerSchema::new(data.for_executing_service()).state_hash()
    }
}
```

See the full implementation of the [service][service], which uses the time oracle.

You can get the time of each validator node in the same manner
the consolidated time of the system is obtained:

```rust
let time_schema = exonum_time::TimeSchema::new(&view);
// Gets the times of all validators.
let validators_time = time_schema.validators_time();
// Gets the time of validator with a public key equal to `public_key`.
let validator_time = time_schema.validators_time().get(&public_key);
```

## Further Reading

Consult [the crate docs](https://docs.rs/exonum-time) for more details about
the service Rust API, and the [service description in Exonum docs](https://exonum.com/doc/version/latest/advanced/time)
for a more high-level perspective, in particular, the design rationale
and the proof of correctness.

## Other languages support

* [Java Time Oracle](https://github.com/exonum/exonum-java-binding/tree/master/exonum-java-binding/time-oracle)

## License

`exonum-time` is licensed under the Apache License (Version 2.0).
See [LICENSE](LICENSE) for details.

[service]: examples/simple_service/main.rs
