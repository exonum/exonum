# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Allow creating auditor node from command line. (#364)

### Fixed
- Fixed `crate_authors!` macro usage, this macro can't return static string in new clap version. (#370)

## 0.3 - 2017-11-02

### Added
- New events implementation based on tokio with the separated queues for network events and timeouts and different threads for the network and node code (#300)
- Add new index `SparseListIndex`. It is a list of items stored in sequential order. Similar to `ListIndex` but it may contain indexes without elements (#312)
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
- Add description of deserialization error for message types (#337)
- Clarify `Transaction.info()` usage (#345)

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
