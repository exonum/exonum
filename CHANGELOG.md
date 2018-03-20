# Changelog

All notable changes to this project will be documented in this file.
The project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Breaking changes

#### Exonum core

- POST-requests are now handled with `bodyparser` crate,
  so all the parameters must be passed in the body. (#529)

- `ProofListIndex` and `ProofMapIndex` `root_hash` method has been renamed to
  `merkle_root`. (#547)

- `with_prefix` constructor of all index types has been renamed to
  `new_in_family`. Now it uses `index_id` instead of prefixes. Moreover,
  `blockchain::gen_prefix` method has been removed. Instead, any type that
  implements `StorageKey` trait, can serve as an `index_id`. (#531)

- Several `Schema`'s methods have been renamed:
  - `tx_location_by_tx_hash` to `transactions_locations`.
  - `block_txs` to `block_transactions`.

- `SystemTime` previously used as storage key or value turned out to show
  different behavior on different platforms and, hence, has been replaced with
  `chrono::DateTime<Utc>` that behaves the same in any environment.

  Migration path:

  - Replace all `SystemTime` fields with `chrono::DateTime<Utc>` ones.
  - Use `DateTime::from` and `into()` methods to convert your existing
  `SystemTime` instances into suitable type when constructing transactions or
  working with database.

#### exonum-testkit

- Testkit api now contains two methods to work with the transaction pool:
  - `is_tx_in_pool` - for checking transaction existence in the pool;
  - `add_tx` - for adding a new transaction into the pool.

  Migration path:

  - Instead of calling `mempool()`, one should use `is_tx_in_pool`
  or `add_tx` methods.

#### exonum-configuration

- `majority_count: Option<u16>` configuration parameter is introduced.
  Allows to increase the threshold amount of votes required to commit
  a new configuration proposal. By default the number of votes is calculated
  as 2/3 + 1 of total validators count. (#546)

#### exonum-time

- `SystemTime` has been replaced with `chrono::DateTime<Utc>`, as it provides
  more predictable behavior on all systems.

### New features

#### Exonum core

- New `database` field added to the `NodeConfig`.
  This optional setting adjusts database-specific settings,
  like number of simultaneously opened files. (#538)

- `exonum::explorer` module moved to the `exonum::api::public`. (#550)

  Migration Path:

  - Rename imports like `exonum::explorer::*` to the `exonum::api::public::*`.

- Added `v1/user_agent` endpoint with information about Exonum, Rust
  and OS versions. (#548)

### Internal improvements

#### Exonum core

- Non-committed transactions are now stored persistently in the storage
  instead of memory pool. (#549)

## 0.6 - 2018-03-06

### Breaking changes

#### Exonum core

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

- `CryptoHash` for `()` now correctly calculates a hash of an empty byte array
  instead of returning `Hash::zero()`. (#483)

- Removed the `'static` bound from the return value of the
  `blockchain::Service::service_name()` method. (#485)

- `StorageKey` trait now requires `ToOwned` implementation. (#392)

- `Connect` message has been extended with a user agent string, which breaks
  binary compatibility with previous versions. (#362)

- Log output become more human-readable. Now it uses `rfc2822` for time formatting.
  This change can break scripts that analyze the log output. (#514)

- `output_dir` argument of the `generate-testnet` command has been renamed to
  `output-dir`. (#528)

- `peer_addr` argument of the `generate-config` command has been renamed to
  `peer-address`. (#528)

- `Blockchain::new` and `Node::new` now accept `Into<Arc<Database>>` instead
  of `Box<Database>`. (#530)

  Migration path:

  - Just pass database argument as is, for example instead of
    `Box::new(MemoryDb::new())` use `MemoryDb::new()`.

#### exonum-configuration

- Most types renamed to avoid stuttering (see [here][stuttering] for
  an explanation of the term) (#496):

  - `ConfigurationService` to `Service`
  - `ConfigurationServiceFactory` to `ServiceFactory`
  - `TxConfigPropose` to `Propose`
  - `TxConfigVote` to `Vote`
  - `ConfigurationSchema` to `Schema`
  - `StorageValueConfigProposeData` to `ProposeData`
  - `CONFIG_SERVICE` constant to `SERVICE_ID`

  Check the crate documentation for more details.

  **Migration path:** Rename imported types from the crate, using aliases
  or qualified names if necessary: `use exonum_configuration::Service as ConfigService`.

[stuttering]: https://doc.rust-lang.org/1.0.0/style/style/naming/README.html#avoid-redundant-prefixes-[rfc-356]

- Multiple APIs are no longer public (#496):

  - Message identifiers
  - Mutating methods of the service schema
  - Module implementing HTTP API of the service

  Check the crate documentation for more details.

  **Migration path:** The restrictions are security-based and should not
  influence intended service use.

<!-- cspell:disable -->

- `ZEROVOTE` is replaced with the `MaybeVote` type, which is now used
  instead of `Vote` in the schema method signatures. The storage format itself
  is unchanged (#496).

<!-- cspell:enable -->

#### exonum-time

- The structure `Time` is removed, use `SystemTime`
  for saving validators time in `ProofMapIndex` instead. (#20)

- Renamed methods `validators_time`/`validators_time_mut` to
  `validators_times`/`validators_times_mut` in `Schema`. (#20)

### New features

#### Exonum core

- `StorageKey` and `StorageValue` traits are implemented for `SystemTime`. (#456)

- `StorageValue` and `CryptoHash` traits are implemented for `bool`. (#385)

- `Height` implements `std::str::FromStr`. (#474)

- `v1/transactions` endpoint has been extended with the transaction execution
  status. (#488)

- Key-indexes interface now allows to use borrowed types for the search
  operations. (#392)

- Added `v1/shutdown` endpoint for graceful node termination. (#526)

- `TransactionInfo` from the public api module became public. (#537)

#### exonum-testkit

- Modified signature of the `TestKitApi::send` method, which previously did not
  accept `Box<Transaction>`. (#505)

- Added possibility to init a logger in `TestKitBuilder`. (#524)

#### exonum-configuration

- Information about configurations by `/v1/configs/actual`, `/v1/configs/following`
  and `/v1/configs/committed` endpoints is extended with the hash of the corresponding
  proposal and votes for the proposal (#481).

- Implemented error handling based on error codes (#496).

### Bug fixes

#### Exonum core

- `ExonumJsonDeserialize` trait is implemented for `F32` and `F64`. (#461)

- Added round and propose timeouts validation. (#523)

- Fixed bug with the extra creation of the genesis configuration. (#527)

- Fixed panic "can't cancel routine" during node shutdown. (#530)

### Internal improvements

#### Exonum core

- Consensus messages are stored persistently (in the database), so restart will
  not affect the node's behavior. (#322)

- Runtime index type checks have been implemented for every index. (#525)

## 0.5.1 - 2018-02-01

### Bug fixes

- Fixed logger output. (#451)

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

  <!-- spell-checker:disable -->

- Fixed typo in `SparceListIndexKeys` and `SparceListIndexValues`. (#398)

  <!-- spell-checker:enable -->

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
- Replaced `rocksdb` command-line parameter to more generic `db-path`. (#376)
- Obsolete trait `HexValue` replaced by the `FromHex` and `ToHex` traits. (#372)
- Changed `Patch` and `Changes` from type definitions into opaque structures. (#371)
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
- Add new type definitions `Height` and `ValidatorId` (#262)
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
- Disallow generate-testnet with 0 nodes (#258)

## 0.1.1 - 2017-09-13

### Fixed

- Fix segfault when `LevelDBSnapshot` is destroyed after `LevelDB` (#285)
- Fix panic during `BlockResponse` message processing
  if the transaction pool is full (#264)
- Fix panic during deserialization of malformed messages (#278 #297)

## 0.1 - 2017-07-17

The first release of Exonum.
