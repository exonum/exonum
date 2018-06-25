# Configuration service tutorial

## Running testnet

### Using Docker

<!-- spell-checker:ignore vitvakatu -->

Simply run the following command to configure and launch testnet with 4 nodes:

```bash
docker run -p 8000-8007:8000-8007 vitvakatu/exonum-configuration-example 4
```

Docker will automatically pull image from the repository and run 4 nodes with
public endpoints at `127.0.0.1:8000`, ..., `127.0.0.1:8003` and
private ones at `127.0.0.1:8004`, ..., `127.0.0.1:8007`.

You can also use helper [script](../docker/example-start.sh):

```bash
./docker/example-start.sh <number of nodes>
```

To stop docker container, use `docker stop <container id>` command.

### Manually

#### Build example binary

To build an [example binary](../examples/configuration.rs) of exonum
blockchain with the single configuration service mounted, run:

```bash
cargo install --example configuration
```

`exonum` crate system dependencies and rust toolchain configuration -
[exonum install instructions](https://exonum.com/doc/get-started/install/).

#### Generate testnet directory and config

`4` is a required indexed parameter and stands for the number of nodes in
testnet:

```bash
mkdir -p testnet/configuration_service
cd testnet/configuration_service
configuration generate-testnet --start 5400 4 --output-dir .
cd ..
```

This should create the following config for testnet:

```bash
$ tree configuration_service/
configuration_service/
└── validators
  ├── 0.toml
  ├── 1.toml
  ├── 2.toml
  └── 3.toml
```

#### Run `4` nodes

<!-- markdownlint-disable MD013 -->

```bash
configuration run --node-config configuration_service/validators/0.toml --db-path configuration_service/db/0 --public-api-address 127.0.0.1:8000 --private-api-address 127.0.0.1:8010
...
configuration run --node-config configuration_service/validators/3.toml --db-path configuration_service/db/3 --public-api-address 127.0.0.1:8003 --private-api-address 127.0.0.1:8013
```

<!-- markdownlint-enable MD013 -->

##### Parameters

- `--public-api-address` is for Exonum [public http api endpoints](#public-endpoints)
- `--private-api-address` is for Exonum [private http api endpoints](#private-endpoints)
- `--node-config` path to the node config
- `--db-path` path to the database

## Global variable service http api

All `hash`es, `public-key`s and `signature`s in tables are hexadecimal strings.
`config-body` is a valid json, corresponding to [exonum config] serialization.

### Public endpoints

#### Configurations' structure

This config is called *actual* config.

1. Only single config may be scheduled to become next config at any moment of
   time. This config is called *following* config.

1. For any current config, its *following* config will have `actual_from`
   greater than the `actual_from` of current config.

1. For any current config, its *following* config will have `previous_cfg_hash`
   equal to hash of current config.

1. Any config propose gets scheduled to become the *following* config only if
   it gets **2/3+1** supermajority of votes of `validators` of *actual* config.
   Thus, which entities can determine what the *following* config will be is
   specified in the contents of *actual* config.

[Examples](response-samples.md#public-response-samples)

<!-- markdownlint-disable MD013 MD033 -->
| Endpoint                                                                                                            | HTTP method   | Description                                                                                                                                                                                                                                                        | Query parameters                                                                                                                                                                                                                                                                                            | Response template                                                                                                                                                                                                                                                   |
| -------------                                                                                                       | ------------- | ------------                                                                                                                                                                                                                                                       | ------------------                                                                                                                                                                                                                                                                                          | ------------------                                                                                                                                                                                                                                                  |
| `/api/services/configuration/v1/configs/actual`                                                                     | GET           | Lookup actual config                                                                                                                                                                                                                                               | None                                                                                                                                                                                                                                                                                                        | {<br>&emsp; "config": **config-body**,<br>&emsp; "hash": **config-hash**<br> }                                                                                                                                                                                      |
| `/api/services/configuration/v1/configs/following`                                                                  | GET           | Lookup already scheduled following config which hasn't yet taken effect.<br> `null` if no config is scheduled                                                                                                                                                      | None                                                                                                                                                                                                                                                                                                        | {<br>&emsp; "config": **config-body**,<br>&emsp; "hash": **config-hash**<br> }                                                                                                                                                                                      |
| `/api/services/configuration/v1/configs/<config-hash>`                                                              | GET           | Lookup config by config hash.<br> If no propose was submitted for a config (genesis config) - "propose" is `null`. <br> If only propose is present, then "committed\_config" is `null`.<br> "propose" key has json-object values, that match **propose-template**. | `<config-hash>` - hash of looked up config.                                                                                                                                                                                                                                                                 | {<br> &emsp;"committed\_config": **config\_body**,<br> &emsp;"propose": {<br> &emsp;&emsp;"num\_validators": **integer**,<br> &emsp;&emsp;"tx\_propose": **propose_transaction_body**, <br> &emsp;"votes\_history\_hash": **vote-history-hash**<br> &emsp;}<br> }        |
| `/api/services/configuration/v1/configs/<config-hash>/votes`                                                        | GET           | Lookup votes for a config propose by config hash.<br> If a vote from validator is absent, `null` returned at the corresponding index in json array. If the config is absent altogether, `null` is returned instead of the array.                                                                                                                | `<config-hash>` - hash of looked up config.                                                                                                                                                                                                                                                                 | [<br> &emsp;&emsp;**vote_for_propose_transaction_body**,<br> &emsp;&emsp;**null**,<br> &emsp;&emsp;...<br> ]                                                                                                                       |
| `/api/services/configuration/v1/configs/committed?previous_cfg_hash=<config-hash>&actual_from=<lowest-actual-from>` | GET           | Lookup all committed configs in commit order.                                                                                                                                                                                                                      | `<previous_cfg_hash>` and `<lowest_actual_from>` are optional filtering parameters.<br> **config-body** is included in response if its *previous\_cfg\_hash* field equals the corresponding parameter. <br>It's included if its *actual\_from* field is greater or equal than corresponding parameter.      | [<br> &emsp;{<br> &emsp;&emsp;"config": **config-body**,<br> &emsp;&emsp;"hash": **config-hash**<br> &emsp;},<br> &emsp;{<br> &emsp;&emsp;"config": **config-body**,<br> &emsp;&emsp;"hash": **config-hash**<br> &emsp;},<br> &emsp;...<br> ]                       |
| `/api/services/configuration/v1/configs/proposed?previous_cfg_hash=<config-hash>&actual_from=<lowest-actual-from>`  | GET           | Lookup all proposed configs in commit order.<br>                                                                                                                                                                                                                   | `<previous_cfg_hash>` and `<lowest_actual_from>` are optional filtering parameters.<br> **propose-template** is included in response if its *previous\_cfg\_hash* field equals the corresponding parameter. <br>It's included if its *actual\_from* field is greater or equal than corresponding parameter. | [<br> &emsp;{<br> &emsp;&emsp;"propose-data": **propose-template**,<br> &emsp;&emsp;"hash": **config-hash**<br> &emsp;},<br> &emsp;{<br> &emsp;&emsp;"propose-data": **propose-template**,<br> &emsp;&emsp;"hash": **config-hash**<br> &emsp;},<br> &emsp;...<br> ] |
<!-- markdownlint-enable MD013 MD033 -->

### Private endpoints

Posting a new config can be performed by any validator maintainer via private
endpoint.

#### Propose and vote transactions restrictions

- Propose transactions will only get submitted and executed with state change
  if all of the following conditions take place:

    1. new config body constitutes a valid json string and corresponds to
       [StoredConfiguration][exonum config] format.

    1. `previous_cfg_hash` in proposed config body equals to hash of *actual*
       config.

    1. `actual_from` in proposed config body is greater than *current height*.
       *current height* is determined as the height of the last
       committed block + 1. This is important to obtain a sequential view of
       configs commit history. And, more important, the linear view of history
       of votes which conditioned scheduling of a config.

    1. a *following* config isn't  already present.

    1. *actual* config contains the node-sender public key in the `validators`
       field array, as specified in `from` field of the propose transaction.
       The `from` field is determined by the public key of the node which
       `postpropose` endpoint is accessed for signing the transaction on
       maintainer's behalf.

    1. propose of config, which evaluates to the same hash, hasn't already
       been submitted.

- Vote transactions will only get submitted and executed with state change if
  all of the following conditions take place:

    1. the vote transaction references a config propose with known config hash.

    1. a *following* config isn't  already present.

    1. *actual* config contains the node-sender's public key in `validators`
       field, as specified in `from` field of vote transaction. The `from`
       field is determined by public key of node whose `postvote` endpoint is
       accessed for signing the transaction on the maintainer's behalf.

    1. `previous_cfg_hash` in the config propose, which is referenced by
       vote transaction, is equal to hash of *actual* config.

    1. `actual_from` in the config propose, which is referenced by vote
       transaction, is greater than *current height*.

    1. no vote from the same node public key has been submitted previously.

[Examples](response-samples.md#private-endpoints-response-samples)

<!-- markdownlint-disable MD013 MD033 -->
| Endpoint                                                                        | HTTP method   | Description                                       | Response template                                                                                 |
| -------------                                                                   | ------------- | ------------                                      | ------------------                                                                                |
| `/api/services/configuration/v1/configs/postpropose`                            | POST          | Post proposed config body                         | {<br> &emsp;"cfg\_hash": **configuration-hash**,<br> &emsp;"tx\_hash": **transaction-hash**<br> } |
| `/api/services/configuration/v1/configs/<config-hash-vote-for>/postvote`        | POST          | Vote for a configuration having specific hash     | {<br> &emsp;"tx\_hash": **transaction-hash**<br> }                                                |
| `/api/services/configuration/v1/configs/<config-hash-vote-against>/postagainst` | POST          | Vote against a configuration having specific hash | {<br> &emsp;"tx\_hash": **transaction-hash**<br> }                                                |
<!-- markdownlint-enable MD013 MD033 -->

[exonum config]: https://docs.rs/exonum/0.5.1/exonum/blockchain/config/struct.StoredConfiguration.html
