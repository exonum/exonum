# Changelog

All notable changes to this project will be documented in this file.
The project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Breaking changes

- `exonum::crypto::CryptoHash` trait is introduced, and `StorageValue::hash`
  and `Message::hash` methods are removed. (#442)

  Migration path:

  - For implementations of `StorageValue`, move the `hash` method
    to `CryptoHash` implementation instead.
  - For implementations of `Message` simply remove `hash` method,
    there's a blanket impl of `CryptoHash` for `Message`.
  - Add `use exonum::crypto::CryptoHash` to use the `hash` method.

- The `StorageKey` trait is re-implemented for signed integer types
  (`i8`, `i16`, `i32` and `i64`) to achieve the natural ordering of produced keys.
  (#443)

  This change will break indices using signed integers as keys.
  To emulate the old implementation, you may create a wrapper around a type
  (e.g., `struct QuirkyI32Key(i32)`) and implement `StorageKey` for it using
  big endian encoding. Then, use the wrapper instead of the int type in indices.
  See the unit tests for `StorageKey` for an example.

- `Transaction::execute` method now returns `TransactionResult` that is stored in
  the blockchain and can be accessed through api. The changes made by transactions
  that return `Err` are discarded. To migrate, add `Ok(())` as the last line to
  the `execute` method. More generally, make sure that the method returns `Ok(())`
  on successful execution. (#385)

- Service transactions are now defined through `transactions!` macro that
  automatically assigns transaction IDs based on the declaration order. (#457)

  Migration path:

  - Move all separate transactions declared as `message!` into one
  `transactions!` macro.
  - Remove `ID` constants.
  - Replace `TYPE` constants with a single `SERVICE_ID` constant.

- Several variants were removed from `ApiError` enum. (#474)

  Migration path:

  - Use generic `ApiError::BadRequest` variant or create `IronError` directly.

- `CommandExtension` uses `failure::Error` instead of `Box<std::error::Error>`
  for errors. (#474)

  Migration path:

  - `std::error::Error` can be converted to `failure::Error` via `.into()` method.

- `storage::Error` implements `failure::Fail` instead of `std::error::Error`. (#474)

### New features

- `StorageKey` and `StorageValue` traits are implemented for `SystemTime`. (#456)
- `StorageValue` and `CryptoHash` traits are implemented for `bool`. (#385)
- `Height` implements `std::str::FromStr`. (#474)

### Bug fixes

- `ExonumJsonDeserialize` trait is implemented for `F32` and `F64`. (#461)

## 0.5.1 - 2018-02-01

### Bug fixes

- Fixed logger output (#451)

## 0.5 - 2018-01-30

### Breaking changes

- The order of bytes and bits in the `DBKey` keys of `ProofMapIndex` became
  consistent. (#419)

  The change influences how Merkle Patricia trees are built
  for `ProofMapIndex`: the bits in each byte of a `DBKey` are now enumerated
  from the least significant bit (LSB) to the most significant bit (MSB),
  compared to MSB-to-LSB ordering used before.
  Note: this change will break old storages using map proofs.

- The `Database` trait is simplified: it is no longer required
  to implement state-sharing `clone` method.
  Instead, the `merge` method now takes a shared reference to `self`. (#422)

- `message!` and `encoding_struct!` no longer require manual `SIZE`
  and offset specification. (#413)

- `from_raw(raw: RawMessage)`  method is moved to the `Message` trait.
  To migrate, add `use exonum::messages::Message`. (#427)

- Changed iterators over `Patch` and `Changes` data into custom types instead
  of standard collection iterators. (#393)

- Fixed typo in `SparceListIndexKeys` and `SparceListIndexValues`. (#398)

- Removed default `state_hash` implementation in the `Service` trait. (#399)

- Removed `info` method from the `Transaction`. (#402)

- Replaced config param `timeout_events_capacity` with
  `internal_events_capacity`. (#388)

- The `Transaction` trait now inherits from `ExonumJson`. (#402)

- Renamed `DBKey` to `ProofPath` and moved a part of its functionality
  to the `BitsRange` trait. (#420)

### New features

- Added `patch` method to the `Fork` structure. (#393)
- Added a public `healthcheck` endpoint. (#405)
- Added serialization support of floating point types through special wrapper
  (`F32` and `F64`). This feature is hidden behind `float_serialize` gate.
  Note: special values (Infinity and NaN) aren't supported. (#384)
- Added a possibility to set maximum message size (`pub max_message_len`
  field in `ConsensusConfig`). (#426)
- Added support for CORS. (#406)
- Added `run-dev` command that performs a simplified node launch
  for testing purposes. (#423)

### Bug fixes

- Fixed consensus on the threshold of 1/3 sleeping validators. (#388)
- Fixed a bunch of inconsistencies and mistakes in the docs. (#439)
- Fixed a bug with message header validation. (#430)

### Internal improvements

- The list of peer connections is now restored to the latest state
  after the process is restarted. (#378)
- Log dependency was updated to 0.4, which can cause issues
  with the previous versions. (#433)
- Better error reporting for configs in the `.toml` format. (#429)

## 0.4 - 2017-12-08

### Added

- Allow creating auditor node from command line. (#364)
- Added a new function `merge_sync`. In this function a write will be flushed
  from the operating system buffer cache
  before the write is considered complete. (#368)
- Added conversion into boxed values for values which implement `Service`
  or `Transaction` traits. (#366)
- Added constructor for the `ServiceContext` which can be useful
  for the alternative node implementations. (#366)
- Implemented `AsRef<RawMessage>` for any Exonum messages that were
  created using the `message!` macro. (#372)
- Implemented additional checks for conversion from raw message. (#372)

### Changed

- Changed a signature of `open` function in a `rocksdb` module.
  `RocksDBOptions` should pass by the reference. (#369)
- `ValidatorState` in the `ServiceContext` replaced by the `ValidatorId`. (#366)
- `add_transaction` in the `ServiceContext` replaced by the `transaction_sender`
  which implements the `TransactionSend` trait. (#366)
- The `Node` constructor now requires `db` and `services` variables
  instead of `blockchain` instance. (#366)
- The `Blockchain` constructor now requires services keypair
  and an `ApiSender` instance. (#366)
- `mount_*_api` methods in `Blockchain` instance now
  do not require `ApiContext`. (#366)
- Rename method `last_height` to `height` in `Schema`. (#379)
- `last_block` now returns `Block` instead of `Option<Block>`. (#379)
- Replaced `rocksdb` commandline parameter to more generic `db-path`. (#376)
- Obsolete trait `HexValue` replaced by the `FromHex` and `ToHex` traits. (#372)
- Changed `Patch` and `Changes` from typedefs into opaque structures. (#371)
- Help text is displayed if required argument is not specified. (#390)

### Removed

- Removed `round` method from the `ServiceContext`. (#366)
- Removed redundant `FromRaw` trait. (#372)
- Removed redundant `current_height` method in `Schema`. (#379)

### Fixed

- Fixed `crate_authors!` macro usage, this macro can't return static string
  in new clap version. (#370)
- Fixed mistake in description of the height getter in the `ServiceContext`. (#366)
- Fixed #15 consensus on the threshold of 1/3 sleeping validators. (#388)

## 0.3 - 2017-11-02

### Added

- New events implementation based on the `tokio` with the separated queues
  for network events and timeouts and different threads for the network
  and node code (#300)
- Added a new index `SparseListIndex`. It is a list of items stored
  in sequential order. Similar to `ListIndex` but it may contain
  indexes without elements (#312)
- Implement `FromStr` and `ToString` traits for public sodium types (#318)
- Add a new macro `metric!` for collecting statistical information (#329)
- Make type `DBKey` public because it is used in `MapProof` (#306)

### Changed

- `RocksDB` is a default storage (#178)
- Field `events_pool_capacity` in `MemoryPoolConfig` replaced
  by the new `EventsPoolCapacity` configuration (#300)
- Changed a build method `new` and added a new build method `with_prefix`
  for indexes (#178)
- Changed a signature of `gen_prefix` function in a `schema` module (#178)
- `NodeBuilder` works with `ServiceFactory` as trait object instead (#357)
- Debug formatting for crypto types are improved (#353)
- Added description of deserialization error for message types (#337)
- Clarified `Transaction.info()` usage (#345)

### Removed

- Support of `LevelDB` is removed (#178)

### Fixed

- Fix the issue causing timeouts are ignored when the event pool is full (#300)
- Fix network failure due to incorrect processing of the incoming buffer (#322)

## 0.2 - 2017-09-13

### Added

- Add `RockDB` support (#273)
- Add `TimeoutAdjusterConfig`, `Constant` and `Dynamic` timeout adjusters (#256)
- Add stream hashing and signing: `HashStream` and `SignStream` (#254)
- Add new typedefs `Height` and `ValidatorId` (#262)
- Fields of `BlockInfo` and `TxInfo` are now public (#283)
- Public export of `PROOF_MAP_KEY_SIZE` constant (#270)

### Changed

- `MapProof` variant fields are renamed: `left_hash` and `right_hash`
  to `left_node` and `right_node` (#286)
- `RequestBlock` is renamed to `BlockRequest` and `Block`
  is renamed to `BlockResponse` (#287)
- All request messages are renamed: `RequestFoo` to `FooRequest` (#287)
- Improve log formatting (#291 #294)
- Make panic message during command line arguments parsing cleaner (#257)

### Fixed

- Fix network discover failure due to incorrect processing
  of the incoming buffer (#299)
- Fix snapshot behavior for `MemoryDB` (#292)
- Dissalow generate-testnet with 0 nodes (#258)

## 0.1.1 - 2017-09-13

### Fixed

- Fix segfault when `LevelDBSnapshot` is destroyed after `LevelDB` (#285)
- Fix panic during `BlockResponse` message processing
  if the transaction pool is full (#264)
- Fix panic during deseralizaion of malformed messages (#278 #297)

## 0.1 - 2017-07-17

The first release of Exonum.
