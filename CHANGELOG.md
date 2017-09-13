# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2017-09-13

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

## [0.1.1] - 2017-09-13

### Fixed
- Fix segfault when `LevelDBSnapshot` is destroyed after `LevelDB` (#285)
- Fix panic during `BlockResponse` message processing if the transaction pool is full (#264)
- Fix panic during deseralizaion of malformed messages (#278 #297)

## [0.1.0] - 2017-07-17

The first release of Exonum.
