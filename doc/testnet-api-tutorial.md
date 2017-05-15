# Configuration service tutorial

## Build instructions

To build an [example binary](examples/configuration.rs) of exonum blockchain 
with the single configuration service mounted, run:

```bash
cargo install --example configuration
```

`exonum` crate system dependencies and rust toolchain configuration - 
[exonum install instructions](https://github.com/exonum/exonum-core/blob/master/INSTALL.md).

## Running testnet

1. Generate testnet dir and testnet config.

  - 4 is a required indexed parameter and stands for number of nodes in testnet:

    ```bash
    mkdir -p testnet/configuration_service
    cd testnet/configuration_service
    configuration generate --output-dir . --start-port 5400 4
    cd ..
    ```

  - This should create following config for testnet:

    ```bash
    $ tree configuration_service/
    configuration_service/
    └── validators
      ├── 0.toml
      ├── 1.toml
      ├── 2.toml
      └── 3.toml
    ```

2. Run 4 nodes:

  - manually for each node process:

    ```bash
    configuration run --node-config configuration_service/validators/0.toml --leveldb-path configuration_service/db/0 --public-port 8000 --private-port 8010
    ...                                                                                                                                                        
    configuration run --node-config configuration_service/validators/3.toml --leveldb-path configuration_service/db/3 --public-port 8003 --private-port 8013
    ```

      - parameters

          - `--public-port` is for configuration service's [public http api 
          endpoints](#public-endpoints)

          - `--private-port` is for configuration service's [private http api 
          endpoints](#private-endpoints)

          - `--node-config` and `--leveldb-path` are described on 
          [exonum install instructions](https://github.com/exonum/exonum-core/blob/master/INSTALL.md)

  - automatically via the [supervisord](http://supervisord.org/) utility.

     1. set the **TESTNET_DESTDIR** environment variable to the `testnet` dir 
        created above:

        ```bash
        $ pwd
        /Users/user/Exonum/testnet
        $ export TESTNET_DESTDIR=/Users/user/Exonum/testnet
        ```

     1. run [helper script](../testnet/testnetctl.sh) for initializing 
        `supervisor` and `configuration_service` process group 
        [config](../testnet/supervisord) to `$TESTNET_DESDIR` directory.
        
        ```bash
        ./testnet/testnetctl.sh enable
        ```

     1. go to `$TESTNET_DESTDIR`. It contains new `etc`, `run`, `log`folders.

        ```bash
        $ cd $TESTNET_DESTDIR
        $ tree .
        .
        ├── configuration_service
        │   └── validators
        │       ├── 0.toml
        │       ├── 1.toml
        │       ├── 2.toml
        │       └── 3.toml
        ├── etc
        │   ├── conf.d
        │   │   └── configuration_service.conf
        │   └── supervisord.conf
        ├── log
        │   ├── supervisor
        │   │   ├── configuration_service_00-stderr---supervisor-rMqmIy.log
        │   │   ... ...                                                          
        │   │   └── configuration_service_03-stdout---supervisor-s29Fd_.log
        │   └── supervisord.log
        └── run
            └── supervisord.pid

        7 directories, 16 files
        ```

     1. launch `configuration_service` process group.

        ```bash
        $ supervisorctl start configuration_service:*
        configuration_service:configuration_service_01: started
        configuration_service:configuration_service_00: started
        configuration_service:configuration_service_03: started
        configuration_service:configuration_service_02: started
        ```

## Global variable service http api

All `hash`es, `public-key`s and `signature`s in tables are hexadecimal 
strings.
`config-body` is a valid json, corresponding to [exonum config](http://exonum.com/doc/crates/exonum/blockchain/config/struct.StoredConfiguration.html) serialization.

### Public endpoints

[Examples](response-samples.md#public-response-samples)

| Endpoint      | HTTP method   | Description |Query parameters|Response template |
| ------------- | ------------- | ------------| ------------------ |------------------ |
| `/api/v1/configs/actual`         | GET | Lookup actual config|      None       |{<br>&emsp; "config": **config-body**,<br>&emsp; "hash": **config-hash**<br> }|
| `/api/v1/configs/following`      | GET | Lookup already scheduled following config which hasn't yet taken effect.<br> `null` if no config is scheduled |    None            |{<br>&emsp; "config": **config-body**,<br>&emsp; "hash": **config-hash**<br> }|
| `/api/v1/configs/<config-hash>` | GET | Lookup config by config hash.<br> If no propose was submitted for a config (genesis config) - "propose" is `null`. <br> If only propose is present, then "committed\_config" is `null`.<br> "propose" key has json-object values, that match **propose-template**.| `<config-hash>` - hash of looked up config.|{<br> &emsp;"committed\_config": **config\_body**,<br> &emsp;"propose": {<br> &emsp;&emsp;"num\_votes": **integer**,<br> &emsp;&emsp;"tx\_propose": {<br> &emsp;&emsp;&emsp;"cfg": **config\_body**,<br> &emsp;&emsp;&emsp;"from": **validator-public-key**,<br> &emsp;&emsp;&emsp;"signature": **validator-signature**<br> &emsp;&emsp;},<br> &emsp;"votes\_history\_hash": **vote-history-hash**<br> &emsp;}<br> }|
| `/api/v1/configs/<config-hash>/votes` | GET | Lookup votes for a config propose by config hash.<br> If a vote from validator is absent - `null` returned at the corresponding index in json array | `<config-hash>` - hash of looked up config. |{<br> &emsp;"Votes": [<br> &emsp;&emsp;{<br> &emsp;&emsp;&emsp;"cfg\_hash": **config-hash**,<br> &emsp;&emsp;&emsp;"from": **validator-public-key**,<br> &emsp;&emsp;&emsp;"signature": **validator-signature**<br> &emsp;&emsp;},<br> &emsp;&emsp;**null**,<br> &emsp;&emsp;...<br> &emsp;]<br> }|
| `/api/v1/configs/committed?previous_cfg_hash=<config-hash>&actual_from=<lowest-actual-from>` | GET | Lookup all committed configs in commit order. |  `<previous_cfg_hash>` and `<lowest_actual_from>` are optional filtering parameters.<br> **config-body** is included in response if its _previous\_cfg\_hash_ field equals the corresponding parameter. <br>It's included if its _actual\_from_ field is greater or equal than corresponding parameter. |[<br> &emsp;{<br> &emsp;&emsp;"config": **config-body**,<br> &emsp;&emsp;"hash": **config-hash**<br> &emsp;},<br> &emsp;{<br> &emsp;&emsp;"config": **config-body**,<br> &emsp;&emsp;"hash": **config-hash**<br> &emsp;},<br> &emsp;...<br> ]|
| `/api/v1/configs/proposed?previous_cfg_hash=<config-hash>&actual_from=<lowest-actual-from>` | GET | Lookup all proposed configs in commit order.<br> |  `<previous_cfg_hash>` and `<lowest_actual_from>` are optional filtering parameters.<br> **propose-template** is included in response if its _previous\_cfg\_hash_ field equals the corresponding parameter. <br>It's included if its _actual\_from_ field is greater or equal than corresponding parameter. |[<br> &emsp;{<br> &emsp;&emsp;"propose-data": **propose-template**,<br> &emsp;&emsp;"hash": **config-hash**<br> &emsp;},<br> &emsp;{<br> &emsp;&emsp;"propose-data": **propose-template**,<br> &emsp;&emsp;"hash": **config-hash**<br> &emsp;},<br> &emsp;...<br> ]|

### Private endpoints

Posting a new config can be performed by any validator maintainer via private 
endpoint.

-   it's important to specify `previous_cfg_hash` in new config body, 
    which should be equal to `hash` of a config, actual at the moment 
    when the new propose is being submitted.

-   `cfg_hash`, returned in response to `postpropose` request, should be used 
    as `<config-hash-vote-for>` parameter of `postvote` request. 

[Examples](response-samples.md#private-response-samples)

| Endpoint      | HTTP method   | Description | Response template |
| ------------- | ------------- | ------------| ------------------ |
| `/api/v1/configs/postpropose`         | POST | Post proposed config body | {<br> &emsp;"cfg\_hash": **configuration-hash**,<br> &emsp;"tx\_hash": **transaction-hash**<br> }|
| `/api/v1/configs/<config-hash-vote-for>/postvote`      | POST | Vote for a configuration having specific hash | {<br> &emsp;"tx\_hash": **transaction-hash**<br> } |
