# Changelog

All notable changes to this project will be documented in this file.
The project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Breaking changes

+ The structure `Time` is removed, use `SystemTime`
for saving validator's time in `ProofMapIndex` instead. (#20)
+ Renamed methods `validators_time`/`validators_time_mut` to
`validators_times`/`validators_times_mut` in `Schema`. (#20)

## 0.5 - 2018-02-01

The first release of Exonum time oracle.
