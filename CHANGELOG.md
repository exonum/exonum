# Changelog

All notable changes to this project will be documented in this file.
The project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## Unreleased

### Breaking Changes

#### exonum

- Node secret keys are now stored in separate files in a secure way.
  CLI for generating node configs and starting nodes has been extended
  in order to reflect these changes. (#1222, #1096, #1235)

- Changed a response for `/healthcheck` endpoint. (#1252)

#### exonum-crypto

- Renamed `create_keys_file` function to `generate_keys_file`
  in `utils` module. (#1222, #1096)

### Internal improvements

- All Exonum crates have been updated to Rust 2018 edition. This means that
  it is required to use Rust 1.31 or newer for compilation. (#1230)

#### exonum

- Added `allow-origin` for `localhost` for public and private api in
  development mode by default. (#1234)

- Added `ProtobufConvert` implementation for `Signature`. (#1241)

## 0.10.3 - 2019-01-22

### Internal Improvements

#### exonum

- Unpin versions of dependencies. (#1237)

## 0.10.2 - 2019-01-14

### New Features

#### exonum

- Added i128/u128 support for `StorageKey`, `StorageValue`. (#1179)

#### exonum-crypto

- Added i128/u128 support for `CryptoHash`. (#1179)

## 0.10.1 - 2019-01-04

### Internal Improvements

#### exonum

- Dependencies have been updated. (#1111, #1162, #1167, #1168)

- `ctrl+c` handler has been added for correct node stopping. (#1163)

#### exonum-crypto

- `pwbox` dependency has been updated. (#1164)

## 0.10.0 - 2018-12-14

### Breaking Changes

- Changed the message format, which, in turn, has led to changes in
   the byte representation of transactions and precommit messages. (#916)

- `Transaction::execute` now accepts `TransactionContext` as the second
   parameter. `TransactionContext` provides the public key of transaction
   author, ID of current service, and transaction hash. (#943)

- `Transaction::verify` method has been removed. (#1085)

- Every transaction that contains the public key of the author was refactored
   to use the author indicated in `TransactionContext`. (#984 #980 #979 #975 #971)

- `/transactions/` endpoint can now handle transaction messages in hex format.
   Services that used custom endpoints were refactored to
   use `explorer/v1/transactions`. (#943 #984 #980 #979 #975 #971)

- `majority_count` parameter has been moved from `StoredConfiguration`
   to `ConfigurationService` configuration. (#828)

- Removed obsolete `enable_blockchain_explorer` option from `NodeApiConfig`. (#891)

- Consensus messages and inner structs are now serialized with protobuf. (#1028)

- `tx_pool_capacity` parameter has been removed from `MemoryPoolConfig`. (#1036)

- Custom serialization has been removed. (#1088)

#### exonum

- Trait `TransactionSend` was removed.
  `ApiSender` now contains `broadcast_transaction` method. (#943)

- `api::Error::InternalError` now contains `failure::Error` instead of
  `Box<::std::error::Error>`. (#879)

- `TransactionSend::send` now returns `Result<(), failure::Error>`
  instead of `io::Result`. (#879)

- `ApiSender` methods `send_external_message` and `peer_add`
   now returns `Result<(), failure::Error>` instead of `io::Result`. (#879)

- `majority_count` parameter has been added to `generate-template` and
  `generate-testnet` commands. (#828)

- `NodePrivateConfig` fields have been renamed: `listen_addr` to `listen_address`
  and `external_addr` to `external_address`. (#809)

- `NodePublicConfig` `addr` field has been renamed to `address`. (#809)

- Config parameter `external_address` is now a required value. (#826)

- Config parameter `round_timeout` has been renamed to `first_round_timeout`.
  Now timeout for round r is `first_round_timeout + (r-1)*round_timeout_increase`,
  where `round_timeout_increase` is determined as a certain percentage of
  `first_round_timeout`. The value of this percentage is defined in
  `ConsensusConfig::TIMEOUT_LINEAR_INCREASE_PERCENT` constant (10%). (#848)

- `missing_keys`, `entries`, `all_entries` methods of `CheckedMapProof` and
  `MapProof::missing_keys_unchecked` method now return `impl Iterator` instead
  of `Vec`. (#918)

- `Connect` message field `addr` with `SocketAddr` has been removed, `pub_addr`
   with `str` of unresolved external address of the peer is used instead. (#942)

- Endpoint `v1/peers` now returns `ConnectInfo` in incoming connections instead
  of single IP-addresses. (#959)

- `Fork::remove_by_prefix()` method now specifies prefix as `Option<&[u8]>` instead
  of `Option<&Vec<u8>>`. (#1042)

- `TransactionResult` is now serialized using protobuf. Empty description
  of the result is now the equivalent of there being no description
  of the result. (#1075)

- `Service::tx_from_raw` now uses `failure::Error` as an error type. (#1088)

- `transactions!` macro has been removed, `TransactionSet` derive macro
  from `exonum-derive` should be used instead. (#1088)

- `encoding_struct!` macro has been removed, protobuf
  should be used instead. (#1088)

#### exonum-testkit

- Structures in tests and examples are serialized using protobuf now. (#1078)

#### exonum-timestamping

- Structures in tests and examples are serialized using protobuf now. (#1081)

#### exonum-cryptocurrency

- Structures in tests and examples are serialized using protobuf now. (#1081)

#### exonum-configuration

- The `Vote` and `VoteAgainst` now save the transaction hash instead of
  full transaction message. (#984)

- Structures are serialized using protobuf now. (#1086)

#### exonum-time

- Structures are serialized using protobuf now. (#1086)

### New Features

#### exonum-crypto

- Added `utils` module with functions `create_keys_file` for creating
  and `read_keys_from_file` for reading files that contain a
  public key and encrypted secret key. (#1056)

#### exonum

- Added possibility to use domain names instead of IP addresses as a peer's
  addresses. (#826)

- Added `v1/rebroadcast` endpoint that can be used to broadcast all transactions
  from the pool to other nodes. (#859)

- Now each consecutive round is longer than previous by some constant percentage
  of `first_round_timeout`. (#848)

- Added `/v1/blocks/subscribe` endpoint for following block commit events
  through WebSockets (#792).

- Added `MapProof::all_entries_unchecked` method. It is used for more efficient
  calculations in Exonum Java Bindings, but can be used for debug purposes
  as well. (#918)

- Added `listen-address` command line argument to specify different external address
  (`peer-address`) and listen address when generating config files. (#942)

- Peer address is resolved on connect instead of resolving on node startup. (#942)

- Now peers require only one connection to exchange messages between
  them. (#945)

#### exonum-build

- `exonum-build` crate has been added to simplify writing `build.rs` files
  for services that use protobuf code generation. (#1076)

#### exonum-derive

- `exonum-derive` crate has been added with custom derives for `ProtobufConvert`
  and `TransactionSet`. (#1055)

### Bug Fixes

#### exonum

- Bug with pool size overflow has been fixed. (#853)

- Bug in `NoiseWrapper::decrypt_msg` caused by wrong calculation of
  encrypted and decrypted message sizes has been fixed. (#873)

- Transactions (signature) verification benchmark has been fixed. (#673)

- Node no longer panics when transaction pool has a lot of transactions and
  consensus is at round 0. (#673)

- Node now works correctly after consensus re-enable via API. (#902)

- Bug with incorrect EOF handling while decoding network messages has been
  fixed. (#917)

- Bug leading to deletion of excessive data when `clear`ing an index belonging
  to an index family has been fixed. (#1042)

### API Improvements

#### exonum

- `new_in_family` index constructor introduced in #531 now accepts `&str` and `&[u8]`
  as an `index_id` value.

### Internal Improvements

#### exonum

- `NodeHandler::run_handler` now returns `Result<(), failure::Error>`
  instead of `io::Result`. (#879)

- Transactions (signature) verification benchmark has been added. (#808)

- A new function `storage::proof_list_index::root_hash()` has been added
  to efficiently compute Merkle root hash from a list of hashes without
  an intermediate `ProofListIndex`. Verification of block root hashes
  has been optimized as well. (#802)

- `NoiseHandshake::finalize` now returns error if remote peer's public key is not
  in `ConnectList`. (#811)

- Now nodes will switch to `min_propose_timeout` for block proposal timeout
  faster if they receive more than `propose_timeout_threshold` transactions
  during `max_propose_timeout`. (#844)

- Custom log formatting (along with `colored` and `term` dependencies) has been
  removed in favor of `env_logger`. (#857).

- Several dependencies have been updated. (#861, #865, #871)

- Transactions are now verified in a thread pool. Thread pool size is set to
  optimal value by default (CPU count) or can be configured manually. (#673)

- The `finalize` command now does not include the node itself as its own
  trusted peer in the generated configuration. (#892)

- Added a possibility to create `ServiceApiBuilder` with blockchain. (#929)

- `ConnectInfo` and `ConnectList` now stores unresolved addresses as strings. (#942)

- Now `network` module uses `PublicKeys` to identify peers. (#942)

- `system/v1/peers` endpoint now properly returns
  incoming and outgoing connections of the node. (#942)

## 0.9.5 - 2018-12-18

### Internal Improvements

#### exonum

- A version of `snow` dependency has been updated.

## 0.9.4 - 2018-10-24

### New Features

#### exonum

- SegmentField implementation for Option has been added, allowing to
  store optional values inside of transactions. (#1004)

## 0.9.3 - 2018-10-04

### Breaking Changes

#### exonum

- Config parameter `external_address` is now a required value. (#826)

### New Features

#### exonum

- Added possibility to use domain names instead of IP addresses as a peer's
  addresses. In config file domain names can be used in `ConnectList`
  configuration and addresses will be resolved once on startup. (#826)

## 0.9.2 - 2018-09-11

### Internal Improvements

#### exonum

- Added a possibility to create `ServiceApiBuilder` with blockchain. (#929)

## 0.9.1 - 2018-08-02

### Bug Fixes

#### exonum

- `failure` version has been updated to `0.1.2` in order to fix the build issue
  with `failure_derive`. (#845)

- Bug with "unknown propose" execution has been fixed. (#841)

## 0.9.0 - 2018-07-19

### Breaking Changes

#### exonum

- `Command` trait in `helpers::fabric` module became public. (#778)

  Migration path:

  If you override `ServiceFactory::command` method and do a match by a command
  name, just add `use helpers::fabric::Command` import.

- `schema_version` field in `Block` has been removed. (#774)

- Storage in exonum is now versioned. Old databases will not work with this
  update. (#707)

- `Iron`-based web API has been replaced by the new implementation based on
  `actix-web`. (#727)

  Migration path:

  For backend:
  - Remove old dependencies on `iron` and its companions `bodyparser`, `router`
    and others.
  - Simplify the API handlers as follows:
    ```rust
    fn my_handler(state: &ServiceApiState, query: MyQueryType)
    -> Result<MyResponse, ApiError>
    {
      // ...
    }
    ```
    where `MyQueryType` type implements `Deserialize` trait and `MyResponse`
    implements `Serialize` trait.
  - Replace old methods `public_api_handler` and `private_api_handler` of
    `Service` trait by a single `wire_api` method which takes
    `ServiceApiBuilder`. You can use this builder as a factory for your service
    API.
  - `get`, `get_err` and `post` methods in `TestKitApi` have been replaced
    by the more convenient `RequestBuilder`.
    Don't forget to update your testkit-based API tests.

  For frontend:
  - New API implementation supports only query parameters in `GET` requests.
    In this way requests like `GET api/my_method/:first/:second`
    should be replaced by the `GET api/my_method?first=value1&second=value2`.
  - JSON parser for `POST` requests is now more strict.
    In this way you should send `null` in request body even for handlers
    without query parameters.

  See our [examples](examples) for more details.

- `storage::base_index` module has become private along with `BaseIndex` and
  `BaseIndexIter` types. (#723)

- `ServiceFactory` trait has been extended with `service_name` function.(#730)

- Method `name` has been removed from `Run`, `GenerateCommonConfig`,
  `GenerateNodeConfig`, `Finalize`, `GenerateTestnet` and `Maintenance`
  structures (`helpers/fabric` module). (#731)

- `Whitelist` has been replaced by `ConnectList`. Now connection between
  nodes can only be established if nodes exist in each other's connect lists.
  (#739)

  Migration path:

  - Replace `[whitelist]` section in config with `[connect_list.peers]` section
  and specify here all validators' consensus public keys with corresponding
  ip-addresses.
  For example `16ef83ca...da72 = "127.0.0.1:6333"`.

- Healthcheck and consensus endpoints (`v1/healthcheck` and
  `v1/consensus_status`) have been merged to `v1/healthcheck`. (#736, #766)

- Node configuration file is now updated at `ConnectList` update. This is
  achieved via new `ConfigManager` entity. (#777)

  Migration path (required only if you create `Node` manually):

  If you need to update `ConnectList` on file system, pass
  `Some(path_to_node_config)` as the last argument of the `Node::new` method.
  Otherwise, pass `None`.

- `exonum::crypto` types now have truncated `Display`/`Debug` representations. (#797)

  Migration path:

  Use `encoding::serialize::ToHex` instead of `Display` to produce full hexadecimal
  representation. You have to manually check if you need to switch or can keep using
  the truncated representation.

  Use `encoding::serialize::FromHex` instead of `FromStr` for reverse conversion.
  `FromStr` implementation has been removed from crypto types to avoid errors.

### New Features

#### exonum

- Existing sodiumoxide-based cryptographic backend behind opt-out
  sodiumoxide-crypto feature. It also allows to use your own cryptographic
  library with exonum. (#756)

- New kind of CLI commands has been added: `info` command that can be used for
  getting various information from a node that has not been started yet. (#731)
  Currently supported sub-commands:
  - `core-version` - prints Exonum version as a plain string.
  - `list-services` - prints the list of the services the node is built with in
    the JSON format.

- `exonum::crypto::x25519` module to convert keys from Ed25519 to X25519 format
  has been introduced. (#722)

- `storage::Entry` has been extended with `take` and `swap` methods. (#781)

- Added remote public key validation when handling incoming `Connect` message. (#786)

### Bug Fixes

#### exonum

- Fixed bug with incorrect peer status for a turned-off node. (#730)

- `handle_consensus` does not write warning for message from previous
  height any more. (#729)

- `new_in_family` constructor has been added to the `Entry` index. (#790)

- Added missing `external_address` field to the auditor final configuration. (#805)

### Internal Improvements

- `BlockResponse` sends transactions by `Hash` instead of `RawMessage`.
  If the node does not have some transactions, requests are created
  for the corresponding transactions. Due to these changes,
  the block size became significantly smaller. (#664)

## 0.8.1 - 2018-06-15

### New Features

#### exonum

- `RunDev` structure has been made public, so it can be extended now.

- `RunDev` command now generates default values for api addresses in the config.

### Internal Improvements

#### exonum

- Dependencies versions have been updated:
  - `exonum_sodiumoxide` to `0.0.19`.
  - `exonum_rocksdb` to `0.7.4`.

## 0.8 - 2018-05-31

### Breaking Changes

#### exonum

- `handle_commit` method in `Service` trait  has been renamed to
  `after_commit`. (#715)

- `TimeoutAdjusterConfig` has been removed along with different timeout
  adjusters. Current behavior is similar to the `Dynamic` timeout adjuster and
  can be modified through `min_propose_timeout`, `max_propose_timeout` and
  `propose_timeout_threshold` fields in the `ConsensusConfig`. (#643)

  Migration path:

  - `Constant` timeout adjuster can be emulated by setting equal
  `min_propose_timeout` and `max_propose_timeout` values.
  - For `Dynamic` timeout adjuster simply move `min`, `max` and `threshold`
    values into `min_propose_timeout`, `max_propose_timeout` and
    `propose_timeout_threshold` correspondingly.
  - There is no possibility to emulate `MovingAverage` now, so `Dynamic` should
    be used as the closest alternative.

- Network connections are now encrypted using
  [Noise Protocol](https://noiseprotocol.org/). Nodes compiled with old
  version will not connect to the new ones. Therefore you need to
  update all node instances for the network to work. (#678)

- `storage::Error` constructor has been made private. (#689)

- `ConsensusConfig::validate_configuration` method has been renamed to the
  `warn_if_nonoptimal`. (#690)

#### exonum-time

- The service has been refactored and the following public structs has been
  moved to separate modules: `TimeSchema` to `exonum_time::schema`,
  `TimeProvider` and `MockTimeProvider` to `exonum_time::time_provider`,
  `ValidatorTime` to `exonum_time::api`. (#604)

### New Features

#### exonum

- Private API now support CORS. (#675)

- The `--public-allow-origin` and `--private-allow-origin` parameters have been
  added to the `finalize` command. (#675)

- IPv6 addressing is now supported. (#615)

- `Field`, `CryptoHash`, `StorageValue` and `ExonumJson` traits have been
  implemented for `chrono::Duration` structure. (#653)

- `before_commit` method has been added in `Service` trait. (#667) (#715)

- `Field`, `CryptoHash`, `StorageKey`, `StorageValue` and `ExonumJson` traits
  have been implemented for `rust_decimal::Decimal`. (#671)

- Maintenance CLI command for node management has been added. Currently the only
  supported command is `clear-cache` which clears node message cache. (#676)

- `StoredConfiguration` validation has been extended with `txs_block_limit`
  parameter check. (#690)

- A warning for non-optimal `StoredConfiguration::txs_block_limit` value has been
  added. (#690)

- Private api `/v1/network/` endpoint now returns core version in addition to
  service info. (#701)

#### exonum-timestamping

- Additional service example has been added along with frontend. (#646)

#### exonum-cryptocurrency-advanced

- Advanced cryptocurrency example becomes a public library (is published on
  crates.io). (#709)

### Bug Fixes

#### exonum

- Already processed transactions are rejected now in
  `NodeHandler::handle_incoming_tx` method. (#642)

- Fixed bug with shutdown requests handling. (#666)

- Fixed deserialization of the `MapProof` data structure. (#674)

- Fixed a bug which prevented the node from reaching the actual round. (#680 #681)

#### exonum-configuration

- Error description has been added to the return value of the transactions. (#695)

#### exonum-time

- Error description has been added to the return value of the transactions. (#695)

#### exonum-cryptocurrency-advanced

- Frontend has been updated to reflect latest backend changes. (#602 #611)

### Internal Improvements

#### exonum

- Default implementation of `check` method was added to `Field` trait to
  reduce boilerplate. (#639)

- Metrics are now using `chrono::DateTime<Utc>` instead of `SystemTime`. (#620)

#### exonum-configuration

- Method `ProposeData::set_history_hash` has been removed. (#604)

## 0.7 - 2018-04-11

### Breaking Changes

#### exonum

- POST-requests are now handled with `bodyparser` crate, so all the parameters
  must be passed in the body. (#529)

- `ProofListIndex` and `ProofMapIndex` `root_hash` method has been renamed to
  `merkle_root`. (#547)

- Proofs of existence / absence for `ProofMapIndex`s have been reworked. They
  now have a linear structure with two components: key-value pairs, and
  additional *proof* information allowing to restore the Merkle root of the
  entire index. `MapProof` interface has been reworked correspondingly. (#380)

  Migration path:

  - Consult documents for the updated workflow for creation and verification
    of `MapProof`s.
  - See the README in the `storage::proof_map_index` module for theoretical
    details about the new proof structure.

- `with_prefix` constructor of all index types has been renamed to
  `new_in_family`. Now it uses `index_id` instead of prefixes. Moreover,
  `blockchain::gen_prefix` method has been removed. Instead, any type that
  implements `StorageKey` trait, can serve as an `index_id`. (#531)

- Several `Schema`'s methods have been renamed (#565):
  - `tx_location_by_tx_hash` to `transactions_locations`.
  - `block_txs` to `block_transactions`.

- `SystemTime` previously used as storage key or value turned out to show
  different behavior on different platforms and, hence, has been replaced with
  `chrono::DateTime<Utc>` that behaves the same in any environment. (#557)

  Migration path:

  - Replace all `SystemTime` fields with `chrono::DateTime<Utc>` ones.
  - Use `DateTime::from` and `into()` methods to convert your existing
  `SystemTime` instances into suitable type when constructing transactions or
  working with database.

- `Blockchain` method `tx_from_raw()` now returns
  `Result<Box<Transaction>, MessageError>` instead of `Option<Box<Transaction>>`.
  (#567)

- `events` module becomes private. (#568)

- `CryptoHash` trait is no longer implemented for `Hash`. (#578)

- `network_id` attribute has been removed from `NodeInfo` and `RawMessage`.
  `HEADER_LENGTH` remains the same, first byte of `RawMessage` is now reserved and
  always set to `0`. (#579)

- `exonum::explorer` module has been reworked to add new functionality.
  (#535, #600) In particular:

  - The explorer now allows to iterate over blocks in the blockchain in the
    given height range, replacing old `blocks_range` method.
  - `block_info` and `tx_info` methods of the explorer are renamed to `block`
    and `transaction` respectively.
  - `TransactionInfo` moved from the `api::public` module to the `explorer` module.
  - `BlocksRange` moved from the `explorer` module to the `api::public` module.
  - `TxInfo` is renamed to `CommittedTransaction`.
  - `BlockInfo` fields are private now, yet accessible with getter methods.

  Migration path:

  - Rename imported types and methods as specified above
  - Use explicit type parameter in `TransactionInfo` and `CommittedTransaction`
    (e.g., `TransactionInfo<serde_json::Value>` or `TransactionInfo<MyTransaction>`)
    if you need to deserialize transaction-related data returned from
    the explorer HTTP API.
  - Consult `explorer` module docs for further possible changes in API.

- `validators-count` command-line parameter has been added. Now, when generating
  config template using `generate-template` command, you must specify the number
  of validators. (#586)

- `majority_count` parameter has been added to the `StoredConfiguration`. See
  `exonum-configuration` changes for more details. (#546)

#### exonum-testkit

- Rollback mechanism in `Testkit` is reworked to work with checkpoints (#582):
  - old `rollback` by blocks in `Testkit` was removed;
  - `checkpoint` method was introduced to set checkpoints;
  - new `rollback` rolls back to the last set checkpoint.

  Migration path:
  - Replace every old `rollback(blocks)` by a pair of `checkpoint()` and `rollback()`.

- Testkit api now contains two methods to work with the transaction pool (#549):
  - `is_tx_in_pool` - for checking transaction existence in the pool;
  - `add_tx` - for adding a new transaction into the pool.

  Migration path:

  - Instead of calling `mempool()`, one should use `is_tx_in_pool`
  or `add_tx` methods.

- `TestKitApi::get_err` method now returns `ApiError`, rather than a deserialized
  object, as it is for `get`. For checking such results
  in tests you may want to use `assert_matches`.

#### exonum-configuration

- `majority_count: Option<u16>` configuration parameter is introduced. Allows to
  increase the threshold amount of votes required to commit a new configuration
  proposal. By default the number of votes is calculated as 2/3 + 1 of total
  validators count. (#546)

#### exonum-time

- `SystemTime` has been replaced with `chrono::DateTime<Utc>`, as it provides
  more predictable behavior on all systems. (#557)

### New Features

#### exonum

- `ExecutionError::with_description` method now takes `Into<String>` instead of
  `String` which allows to pass `&str` directly. (#592)

- New `database` field added to the `NodeConfig`. This optional setting adjusts
  database-specific settings, like number of simultaneously opened files. (#538)

- `ExecutionError::with_description` method now takes `Into<String>` instead of
  `String` which allows to pass `&str` directly. (#592)

- New `database` field added to the `NodeConfig`. This optional setting adjusts
  database-specific settings, like number of simultaneously opened files. (#538)

- Added `v1/user_agent` endpoint with information about Exonum, Rust and OS
  versions. (#548)

- `ProofMapIndex` now allows to retrieve a proof of presence / absence for an
  arbitrary number of elements at one time with the help of `get_multiproof`
  method. Correspondingly, `MapProof` allows to verify proofs for an arbitrary
  number of elements. (#380)

- `storage::UniqueHash` trait that represents a unique, but not necessary
  cryptographic hash function, is introduced. (#579)

- Added the opportunity to parse configuration files with missing empty structures.
  Fields of such structures are equal to the default values. (#576)

- `CryptoHash`, `Field`, `StorageKey` and `StorageValue` traits are implemented for
  the `uuid::Uuid`. (#588)

- `Display` trait is implemented for types from the `crypto` module. (#590)

- `transactions!` macro now allows empty body. (#593)

#### exonum-testkit

- `create_block*` methods of the `TestKit` now return the information about
  the created block. (#535)

- `TestKit::explorer()` method allows to access the blockchain explorer. (#535)

#### exonum-cryptocurrency-advanced

- A more complex example has been added featuring best practices for service
  writing. (#595)

### Internal Improvements

#### exonum

- `RawTransaction` now has its own implementation of `fmt::Debug` trait instead
  of `#[derive(Debug)]`. The template of `RawTransaction`’s debug message is
  `Transaction { version: #, service_id: #, message_type: #, length: #,
  hash: Hash(###) }`. (#603)

- Non-committed transactions are now stored persistently in the storage instead
  of memory pool. (#549)

- Sandbox tests have been moved inside of the exonum core. (#568)

- The requested transactions in the `TransactionsRequest` are now sent by batches,
  rather than one by one. The number of batches depends on the size limits
  of the message. (#583)

#### exonum-testkit

- Request logging for `TestKitApi` now encompasses all requests. The log format
  is slightly changed to allow for the generic request / response form. (#601)

## 0.6 - 2018-03-06

### Breaking Changes

#### exonum

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

### New Features

#### exonum

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

### Bug Fixes

#### exonum

- `ExonumJsonDeserialize` trait is implemented for `F32` and `F64`. (#461)

- Added round and propose timeouts validation. (#523)

- Fixed bug with the extra creation of the genesis configuration. (#527)

- Fixed panic "can't cancel routine" during node shutdown. (#530)

### Internal Improvements

#### exonum

- Consensus messages are stored persistently (in the database), so restart will
  not affect the node's behavior. (#322)

- Runtime index type checks have been implemented for every index. (#525)

## 0.5.1 - 2018-02-01

### Bug Fixes

- Fixed logger output. (#451)

## 0.5 - 2018-01-30

### Breaking Changes

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

### New Features

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

### Bug Fixes

- Fixed consensus on the threshold of 1/3 sleeping validators. (#388)
- Fixed a bunch of inconsistencies and mistakes in the docs. (#439)
- Fixed a bug with message header validation. (#430)

### Internal Improvements

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
