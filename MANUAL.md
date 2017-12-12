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

The primary goal of this kind of tests is to check the business logic of your service.

For writing your first test create `tests` directory according to the cargo
integration testing [manual][integration-tests].
After that, create file `tests/transactions.rs` with the content similar to written below.

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
    testkit.create_block_with_transactions(txvec![tx]);
    // Check the expected result.
    let snapshot = testkit.snapshot();
    let schema = MySchema::new(&snapshot);
    assert!(schema.is_my_data_checked());
}
```

Make sure that you have full coverage of the business logic in the `execute` method of your transactions.
But if you just want to check the `verify` logic in the transaction, you can do it without testkit in
a simple way like this:

```rust
let tx = MyTransaction::new(...);
assert!(tx.verify());
```

Testkit also allows to check different orderings of transactions, including transactions for multiple services. This could allow to more efficiently test margin cases, which would be difficult to reproduce otherwise.

```rust
let mut testkit = TestKitBuilder::validator()
    .with_service(MyService::new())
    .with_service(OtherService::new())
    .create();
// Create transactions.
let tx1 = MyTransaction::new(...);
let tx2 = OtherTransaction::new(...);
// Commit its into blockchain.
testkit.create_block_with_transactions(txvec![tx1, tx2]);
// Check the expected result.
```

### API testing

The following steps may help you.

* Define the `MyServiceApi` trait for the `TestKitApi` structure that covers the whole api of your service.
* Implement functions that use transactions to fill your storage with the test data.
* Create the tests that check all of your endpoints.

```rust

// API trait definition.

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

// API test skeleton

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

Here are examples of more complex and less common cases.

### Oracles testing

The oracle in this case is a service which can produce transactions with external data after the commit of the block,
[`exonum-time`][exonum-time] and [`exonum-btc-anchoring`][exonum-btc-anchoring] are examples of these kind of oracles.
In this way, transactions created during the `handle_commit` execution will be stored in `TestKit` memory pool.

```rust
let mut testkit = TestKitBuilder::validator()
    .with_service(HandleCommitService)
    .create();
// Check that `handle_commit` has been invoked at the correct height.
for i in 1..5 {
    testkit.create_block();
    let tx = TxAfterCommit::new_with_signature(Height(i), &Signature::zero());
    assert!(testkit.mempool().contains_key(&tx.hash()));
}
```

In order to invoke a `handle_commit` event, you must create a block.
If the oracle has to fetch any data from external world, you must create a mock object.

```rust
// Provide a mock api for the service.
let mut cruel_world = ExternalApiMock::new();
let mut testkit = TestKitBuilder::validator()
    .with_service(MyOracleService::with_client(cruel_world.client()))
    .create();
// Expect a request from the service.
cruel_world.expect_api_call(ApiCallInfo { ... })
    .with_response_ok(ApiResponse { ... });
// Call the `handle_commit` event.
testkit.create_block();
let expected_tx = MyOracleTx::new(...);
// Check that the expected transaction is in the memory pool.
assert!(testkit.mempool().contains_key(&expected_tx.hash()));
```

### Configuration changes testing

If your service has its own configuration, you may need to test the response to a configuration change.
With the testkit you can create the configuration change proposal and commit it.

```rust
// Provide a mock api for the service.
let mut testkit = TestKitBuilder::validator()
    .with_service(MyOracleService::new())
    .create();
// Create a configuration change proposal.
let proposal = {
    let mut cfg = testkit.configuration_change_proposal();
    cfg.set_actual_from(cfg_change_height);
    cfg.set_service_config("my_service", MyServiceCfg { ... });
    cfg
};
let stored = proposal.stored_configuration().clone();
testkit.commit_configuration_change(proposal);
// Check that the following configuration is none.
use exonum::blockchain::Schema;
assert_eq!(
    Schema::new(&testkit.snapshot()).following_configuration(),
    None
);
testkit.create_block();
// Check that the following configuration is now scheduled.
assert_eq!(
    Schema::new(&testkit.snapshot()).following_configuration(),
    Some(stored)
);
```

If your service has some business logic in the `handle_commit` event handler, you can check it
in the same way as provided in the previous paragraph.

[integration-tests]: https://doc.rust-lang.org/book/second-edition/ch11-03-test-organization.html#integration-tests
[exonum-btc-anchoring]: https://github.com/exonum/exonum-btc-anchoring
[exonum-time]: https://github.com/exonum/exonum-time