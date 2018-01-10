# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Added `patch` method to the `Fork` structure. (#393)
- Added a public `helthcheck` endpoint. (#405)
- Added serialization support floating point types through special wrapper(`F32` and `F64`). This feature is hidden behind `float_serialize` gate. Note: special values (Infinity and NaN) aren't supported. (#384)

### Changed
- Changed iterators over `Patch` and `Changes` data into custom types instead of standard collection iterators. (#393)
- Fixed typo in `SparceListIndexKeys` and `SparceListIndexValues` (#398)
- Fixed #15 consensus on the threshold of 1/3 sleeping validators. (#388)
- Replaced config param `timeout_events_capacity` with `internal_events_capacity`. (#388)
- The `Transaction` trait now inherit `ExonumJson`. (#402)
- The list of peer connections is now restored to the last state after the process is restarted. (#378)
- `message!` and `encoding_struct!` no longer require manual `SIZE` and offset specification.

### Removed
- Removed default `state_hash` implementation in the `Service` trait. (#399)
- Removed `info` method from the `Transaction`. (#402)

## 0.4 - 2017-12-08

### Added
- Allow creating auditor node from command line. (#364)
- Added a new function `merge_sync`. In this function a write will be flushed from the operating system buffer cache before the write is considered complete. (#368)
- Added conversion into boxed values for values which implement `Service` or `Transaction` traits. (#366)
- Added constructor for the `ServiceContext` which can be useful for the alternative node implementations. (#366)
- Implemented `AsRef<RawMessage>` for any Exonum messages that were created using the `message!` macro. (#372)
- Implemented additional checks for conversion from raw message. (#372)

### Changed
- Changed a signature of `open` function in a `rocksdb` module. `RocksDBOptions` should pass by the reference. (#369)
- `ValidatorState` in the `ServiceContext` replaced by the `ValidatorId`. (#366)
- `add_transaction` in the `ServiceContext` replaced by the `transaction_sender` which implements the `TransactionSend` trait. (#366)
- The `Node` constructor now requires `db` and `services` variables instead of `blockchain` instance. (#366)
- The `Blockchain` constructor now requires services keypair and an `ApiSender` instance. (#366)
- `mount_*_api` methods in `Blockchain` instance now do not require `ApiContext`. (#366)
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
- Fixed `crate_authors!` macro usage, this macro can't return static string in new clap version. (#370)
- Fixed mistake in description of the height getter in the `ServiceContext`. (#366)
- Fixed #15 consensus on the threshold of 1/3 sleeping validators. (#388)

## 0.3 - 2017-11-02

### Added
- New events implementation based on the `tokio` with the separated queues for network events and timeouts and different threads for the network and node code (#300)
- Added a new index `SparseListIndex`. It is a list of items stored in sequential order. Similar to `ListIndex` but it may contain indexes without elements (#312)
- Implement `FromStr` and `ToString` traits for public sodium types (#318)
- Add a new macro `metric!` for collecting statistical information (#329)
- Make type `DBKey` public because it is used in `MapProof` (#306)

### Changed
- `RocksDB` is a default storage (#178)
- Field `events_pool_capacity` in `MemoryPoolConfig` replaced by the new `EventsPoolCapacity` configuration (#300)
- Changed a build method `new` and added a new build method `with_prefix` for indexes (#178)
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
- `MapProof` variant fields are renamed: `left_hash` and `right_hash` to `left_node` and
  `right_node` (#286)
- `RequestBlock` is renamed to `BlockRequest` and `Block` is renamed to `BlockResponse` (#287)
- All request messages are renamed: `RequestFoo` to `FooRequest` (#287)
- Improve log formatting (#291 #294)
- Make panic message during command line arguments parsing cleaner (#257)

### Fixed
- Fix network discover failure due to incorrect processing of the incoming buffer (#299)
- Fix snapshot behavior for `MemoryDB` (#292)
- Dissalow generate-testnet with 0 nodes (#258)

## 0.1.1 - 2017-09-13

### Fixed
- Fix segfault when `LevelDBSnapshot` is destroyed after `LevelDB` (#285)
- Fix panic during `BlockResponse` message processing if the transaction pool is full (#264)
- Fix panic during deseralizaion of malformed messages (#278 #297)

## 0.1 - 2017-07-17

The first release of Exonum.
