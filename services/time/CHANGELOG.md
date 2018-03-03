# Changelog

All notable changes to this project will be documented in this file.
The project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

## 0.6 - 2018-03-04

### Breaking changes

- The structure `Time` is removed, use `SystemTime`
  for saving validators time in `ProofMapIndex` instead. (#20)

- Renamed methods `validators_time`/`validators_time_mut` to
  `validators_times`/`validators_times_mut` in `Schema`. (#20)

### New features

- Update to the [Exonum 0.6.0] release (#533).

## 0.5 - 2018-02-01

The first release of Exonum time oracle.

[Exonum 0.6.0]: https://github.com/exonum/exonum/releases/tag/v0.6
