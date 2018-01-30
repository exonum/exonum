# exonum-time

[![Build Status](https://travis-ci.com/exonum/exonum-time.svg?branch=master)](https://travis-ci.com/exonum/exonum-time)

Exonum-time is a time oracle service for [Exonum blockchain framework](https://exonum.com/).
This service allows to determine time, 
import it from the external world to the blockchain 
and keep its current value in the blockchain.

## Usage

Add the following line to the `Cargo.toml`:

```toml
[dependencies]
exonum-time = "0.1.0"
```

And activate service in the main project file:

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

Typical usage of the service boils down to importing the schema and calling its `time()` or `validators_time()` methods.

Below is an example of a method for processing a transaction, 
which must be executed no later than the specified time 
(this time is written in the transaction body by in a separate field):

```rust
message! {
    struct Tx {
        ...
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
            Some(ref current_time) if current_time.time() > self.time() => {
                return;
            }
            _ => { ... }
        }
        ...
    }
    ... 
}
```

You can get the time of each validator node in the same manner the consolidated time of the system is obtained:

```rust
let time_schema = exonum_time::TimeSchema::new(&view);
let validators_time = time_schema.validators_time();
```

See the full implementation of the [`service`][service], which uses the time oracle.
For testing the service [`exonum-testkit`][exonum-testkit] is used.

## License

`exonum-time` is licensed under the Apache License (Version 2.0). See [LICENSE][license] for details.

[service]: examples/simple_service.rs
[exonum-testkit]: https://github.com/exonum/exonum-testkit
[license]: https://github.com/exonum/exonum-time/blob/master/LICENSE
