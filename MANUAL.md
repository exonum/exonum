# Testkit manual

Testkit for Exonum blockchain framework, allowing to test service APIs synchronously and in the same process as the testkit.

Below is a simple user guide.

* [Installation](#installation)
* [Transactions testing](#transactions-testing)
* [Api testing](#api-testing)
* [Oracles testing](#oracles-testing)
* [Configuration changes testing](#configuration-changes-testing)

## Installation

Just add a following line to the `Cargo.toml`:

```toml
[dev-dependencies]
exonum-testkit = "0.1.0"
```

## Simple usage

### Transactions testing

The primary goal of the kind of tests is to check the business logic of your service.

For writting your first test create `tests` directory in according of cargo integration testing [manual][integration-tests].
After that create file `transactions.rs` like the following content.

```rust
extern crate exonum;
extern crate exonum_testkit;
extern crate my_service;

use my_service::{MyService, MyTransaction, MySchema};
use exonum_testkit::TestKitBuilder;

#[test]
fn test_my_tx() {
    // Create simple testkit network.
    let mut testkit = TestKitBuilder::validator()
        .with_service(MyService::new())
        .create();
    // Create transaction.
    let tx = MyTransaction::new(...);
    // Commit it into blockchain.
    testkit.create_block_with_transactions(txvec![tx.clone()]);
    // Check the expected result
    let snapshot = testkit.snapshot();
    let schema = MySchema::new(&snapshot);
    assert!(schema.is_my_data_checked());
}
```

Make sure that you have full coverage of the business logic in the `execute` method of your transactions.
But if you just want to check the `verify` logic in transaction, you can do it without testkit in simple way like this:

```rust
let tx = MyTransaction::new(...);
assert!(tx.verify());
```

### Api testing

The following steps may helps you.

* Define the `MyServiceApi` trait for the `TestKitApi` structure that covers the whole api of your service.
* Implement functions that uses transactions to fill your storage with the test data.
* Create the tests that check all of your endpoints.

```rust

// Api trait definition.

trait MyServiceApi {
    fn get_public_data(&self) -> ApiResponsePublicData;
    fn get_private_data(&self) -> ApiResponsePrivateData;
    fn post_private_data(&self, data: MyPrivateData) -> ApiResponsePostPrivateData;
}

impl MyServiceApi for TestKitApi {
    fn get_public_data(&self) -> ApiResponsePublicData {
        self.get(ApiKind::Service("my_service"), "/v1/first_endpoint")
    }

    fn get_private_data(&self) -> ApiResponsePublicData {
        self.get_private(ApiKind::Service("my_service"), "/v1/second_endpoint")
    }

    fn post_private_data(&self, data: &MyPrivateData) -> ApiResponsePostPrivateData {
        self.post(
            ApiKind::Service("my_service"),
            "v1/third_endpoint",
            &data,
        )
    }
}

// Api test skeleton

#[test]
fn my_api_test() {
    let mut testkit = TestKitBuilder::validator()
        .with_service(MyService::new())
        .create();
    fill_storage_with_data(&mut testkit);
    // Check api responses
    let api = testkit.api();
    assert_eq!(api.get_public_data(), ApiResponsePublicData::new(...));
    ...
}
```

## Advanced usage

### Oracles testing

### Configuration changes testing

[integration-tests]: https://doc.rust-lang.org/book/second-edition/ch11-03-test-organization.html#integration-tests
