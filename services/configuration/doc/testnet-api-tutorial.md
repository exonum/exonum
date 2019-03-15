# Configuration service tutorial

## Running testnet

### Using Docker

<!-- spell-checker:ignore vitvakatu -->

Simply run the following command to configure and launch testnet with 4 nodes:

```bash
docker run -p 8000-8007:8000-8007 \
exonumhub/exonum-configuration-service:example 4
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

#### Generate configs

Generate template:

```sh
mkdir example
configuration generate-template example/common.toml --validators-count 4
```

Generate templates of nodes configurations:
<!-- markdownlint-disable MD013 -->

```sh
configuration generate-config example/common.toml  example/pub_1.toml example/sec_1.toml --peer-address 127.0.0.1:6331 -c example/consensus_1.toml -s example/service_1.toml -n

configuration generate-config example/common.toml  example/pub_2.toml example/sec_2.toml --peer-address 127.0.0.1:6332 -c example/consensus_2.toml -s example/service_2.toml -n

configuration generate-config example/common.toml  example/pub_3.toml example/sec_3.toml --peer-address 127.0.0.1:6333 -c example/consensus_3.toml -s example/service_3.toml -n

configuration generate-config example/common.toml  example/pub_4.toml example/sec_4.toml --peer-address 127.0.0.1:6334 -c example/consensus_4.toml -s example/service_4.toml -n
```

Finalize generation of nodes configurations:

```sh
configuration finalize --public-api-address 0.0.0.0:8200 --private-api-address 0.0.0.0:8091 example/sec_1.toml example/node_1_cfg.toml --public-configs example/pub_1.toml example/pub_2.toml example/pub_3.toml example/pub_4.toml

configuration finalize --public-api-address 0.0.0.0:8201 --private-api-address 0.0.0.0:8092 example/sec_2.toml example/node_2_cfg.toml --public-configs example/pub_1.toml example/pub_2.toml example/pub_3.toml example/pub_4.toml

configuration finalize --public-api-address 0.0.0.0:8202 --private-api-address 0.0.0.0:8093 example/sec_3.toml example/node_3_cfg.toml --public-configs example/pub_1.toml example/pub_2.toml example/pub_3.toml example/pub_4.toml

configuration finalize --public-api-address 0.0.0.0:8203 --private-api-address 0.0.0.0:8094 example/sec_4.toml example/node_4_cfg.toml --public-configs example/pub_1.toml example/pub_2.toml example/pub_3.toml example/pub_4.toml
```

#### Run nodes

```sh
configuration run --node-config example/node_1_cfg.toml --db-path example/db1 --public-api-address 0.0.0.0:8200 --consensus-key-pass pass --service-key-pass pass

configuration run --node-config example/node_2_cfg.toml --db-path example/db2 --public-api-address 0.0.0.0:8201 --consensus-key-pass pass --service-key-pass pass

configuration run --node-config example/node_3_cfg.toml --db-path example/db3 --public-api-address 0.0.0.0:8202 --consensus-key-pass pass --service-key-pass pass

configuration run --node-config example/node_4_cfg.toml --db-path example/db4 --public-api-address 0.0.0.0:8203 --consensus-key-pass pass --service-key-pass pass
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
| `/api/services/configuration/v1/configs?hash=<config-hash>`                                                         | GET           | Lookup config by config hash.<br> If no propose was submitted for a config (genesis config) - "propose" is `null`. <br> If only propose is present, then "committed\_config" is `null`.<br> "propose" key has json-object values, that match **propose-template**. | `<config-hash>` - hash of looked up config.                                                                                                                                                                                                                                                                 | {<br> &emsp;"committed\_config": **config\_body**,<br> &emsp;"propose": {<br> &emsp;&emsp;"num\_validators": **integer**,<br> &emsp;&emsp;"tx\_propose": **propose_transaction_body**, <br> &emsp;"votes\_history\_hash": **vote-history-hash**<br> &emsp;}<br> }        |
| `/api/services/configuration/v1/configs/votes?hash=<config-hash>`                                                   | GET           | Lookup votes for a config propose by config hash.<br> If a vote from validator is absent, `null` returned at the corresponding index in json array. If the config is absent altogether, `null` is returned instead of the array.                                                                                                                | `<config-hash>` - hash of looked up config.                                                                                                                                                                                                                                                                 | [<br> &emsp;&emsp;**vote_for_propose_transaction_body**,<br> &emsp;&emsp;**null**,<br> &emsp;&emsp;...<br> ]                                                                                                                       |
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
| Endpoint                                             | HTTP method   | Description                                       | Response template                                                                                 |
| -------------                                        | ------------- | ------------                                      | ------------------                                                                                |
| `/api/services/configuration/v1/configs/postpropose` | POST          | Post proposed config body                         | {<br> &emsp;"cfg\_hash": **configuration-hash**,<br> &emsp;"tx\_hash": **transaction-hash**<br> } |
| `/api/services/configuration/v1/configs/postvote`    | POST          | Vote for a configuration having specific hash     | {<br> &emsp;"tx\_hash": **transaction-hash**<br> }                                                |
| `/api/services/configuration/v1/configs/postagainst` | POST          | Vote against a configuration having specific hash | {<br> &emsp;"tx\_hash": **transaction-hash**<br> }                                                |
<!-- markdownlint-enable MD013 MD033 -->

[exonum config]: https://docs.rs/exonum/0.5.1/exonum/blockchain/config/struct.StoredConfiguration.html
