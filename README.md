# Exonum configuration/[global variables] service &emsp; [![Build Status](https://travis-ci.com/exonum/exonum-configuration.svg?token=ygdqGfZjj1YKhGQQzBzp&branch=master)](https://travis-ci.com/exonum/exonum-configuration) 
This crate implements functionality of modifying `Exonum` blockchain global configuration via exchanging transactions with 
- config proposes and 
- votes for a specific config propose. 

Configuration is comprised of: 

1. Consensus algorithm parameters. 
2. List of validators' public keys. 
3. Services' configuration. 

## You may be looking for: 
* [Reference documentation](http://exonum.com/doc/crates/configuration_service/index.html)
* [Example code](examples/configuration.rs)
* [Testnet tutorial](doc/testnet_api_tutorial.md)

# Usage
See [tutorial](doc/testnet_api_tutorial.md) for details.

# Licence
Configuration service licensed under [Apache License, Version 2.0](LICENSE).
