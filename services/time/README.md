# exonum-time

[![Travis Build Status](https://img.shields.io/travis/exonum/exonum/master.svg?label=Linux%20Build)](https://travis-ci.org/exonum/exonum)
![CircleCI Build Status](https://img.shields.io/circleci/project/github/exonum/exonum.svg?label=MacOS%20Build)
[![Docs.rs](https://docs.rs/exonum-time/badge.svg)](https://docs.rs/exonum-time)
[![License: Apache-2.0](https://img.shields.io/github/license/exonum/exonum.svg)](https://github.com/exonum/exonum/blob/master/LICENSE)
![rust 1.26.1+ required](https://img.shields.io/badge/rust-1.26.1+-blue.svg?label=Required%20Rust)

Exonum-time is a time oracle service for [Exonum blockchain framework](https://exonum.com/).
This service allows to determine time,
import it from the external world to the blockchain
and keep its current value in the blockchain.

## Usage

Include `exonum-time` as a dependency in your `Cargo.toml`:

```toml
[dependencies]
exonum-time = "0.9.0"
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
message! {
    struct Tx {
        time: SystemTime,
        ...
    }
}

impl Transaction for Tx {
    ...
    fn execute(&self, view: &mut Fork) {
        // Import schema.
        let time_schema = exonum_time::TimeSchema::new(&view);
        // The time in the transaction should be less than in the blockchain.
        match time_schema.time().get() {
            Some(current_time) if current_time <= self.time() => {
                // Execute transaction business logic.
            }
            _ => {}
        }
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

## Further reading

Consult [the crate docs](https://docs.rs/exonum-time) for more details about
the service Rust API, and the [service description in Exonum docs](https://exonum.com/doc/advanced/time)
for a more high-level perspective, in particular, the design rationale
and the proof of correctness.

## License

`exonum-time` is licensed under the Apache License (Version 2.0).
See [LICENSE](LICENSE) for details.

[service]: examples/simple_service.rs
