# Changelog

All notable changes to this project will be documented in this file.
The project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Breaking changes

- Most types renamed to avoid [stuttering] (#496):

  - `ConfigurationService` to `Service`
  - `ConfigurationServiceFactory` to `ServiceFactory`
  - `TxConfigPropose` to `Propose`
  - `TxConfigVote` to `Vote`
  - `ConfigurationSchema` to `Schema`
  - `StorageValueConfigProposeData` to `ProposeData`
  - `CONFIG_SERVICE` constant to `SERVICE_ID`

  Check the crate documentation for more details.

[stuttering]: https://doc.rust-lang.org/1.0.0/style/style/naming/README.html#avoid-redundant-prefixes-[rfc-356]

- Multiple APIs are no longer public (#496):

  - Message identifiers
  - Mutating methods of the service schema
  - Module implementing HTTP API of the service

  Check the crate documentation for more details.

- `ZEROVOTE` structure is replaced with the `MaybeVote` type, which is now used
  instead of `Vote` in the schema method signatures. The storage format itself
  is unchanged (#496).

### New features

- Implemented error handling based on error codes (#496).

## 0.5 - 2018-01-30

- Update to the [Exonum 0.5.0] release (#82).

## 0.4 - 2017-12-08

- Added tests written on `exonum-testkit` (#69).

- Separate type `ConfigurationServiceFactory` is used as `ServiceFactory`
  implementation (#66).

- Update to the [Exonum 0.4.0] release (#77).

- Sandbox tests are removed (#69).

## 0.3 - 2017-11-03

- Update to the [Exonum 0.3.0] release (#65).

## 0.2 - 2017-09-14

- Update to the [Exonum 0.2.0] release (#61).

## 0.1 - 2017-07-17

The first release of Exonum.

[Exonum 0.2.0]: https://github.com/exonum/exonum/releases/tag/v0.2
[Exonum 0.3.0]: https://github.com/exonum/exonum/releases/tag/v0.3
[Exonum 0.4.0]: https://github.com/exonum/exonum/releases/tag/v0.4
[Exonum 0.5.0]: https://github.com/exonum/exonum/releases/tag/v0.5
