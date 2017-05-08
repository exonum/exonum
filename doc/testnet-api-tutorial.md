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
    configuration run --node-config configuration_service/validators/1.toml --leveldb-path configuration_service/db/1 --public-port 8001 --private-port 8011
    configuration run --node-config configuration_service/validators/2.toml --leveldb-path configuration_service/db/2 --public-port 8002 --private-port 8012
    configuration run --node-config configuration_service/validators/3.toml --leveldb-path configuration_service/db/3 --public-port 8003 --private-port 8013
    ```

      - parameters

          - `--public-port` is for configuration service's [public http api 
          endpoints](#public-endpoints)

          - `--private-port` is for configuration service's private http api 
          endpoints

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
        │   │   ├── configuration_service_00-stdout---supervisor-BWDVqD.log
        │   │   ├── configuration_service_01-stderr---supervisor-2NgHVG.log
        │   │   ├── configuration_service_01-stdout---supervisor-Qcyr2v.log
        │   │   ├── configuration_service_02-stderr---supervisor-olCKCx.log
        │   │   ├── configuration_service_02-stdout---supervisor-F2IdNB.log
        │   │   ├── configuration_service_03-stderr---supervisor-Z3MxXS.log
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

### Public endpoints

All `hash`es, `public-key`s and `signature`s in the table are hexadecimal 
strings.

[Examples](#public-response-samples)

| Endpoint      | HTTP method   | Description | Response template |
| ------------- | ------------- | ------------| ------------------ |
| `/api/v1/config/actual`         | GET | Lookup actual config| {<br>&emsp; "config": **config-body**,<br>&emsp; "hash": **configuration-hash**<br> }|
| `/api/v1/config/following`      | GET | Lookup already accepted following config which hasn't yet taken effect<br> `null` if no scheduled config|  {<br>&emsp; "config": **config-body**,<br>&emsp; "hash": **configuration-hash**<br> }|
| `/api/v1/configs/<config-hash>` | GET | Lookup config by config hash<br> If no propose was submitted for a config (genesis config) - "propose" is `null`<br> If only propose is present, then "committed\_config" is `null`| {<br> &emsp;"committed\_config": **config\_body**,<br> &emsp;"propose": {<br> &emsp;&emsp;"num\_votes": **integer**,<br> &emsp;&emsp;"tx\_propose": {<br> &emsp;&emsp;&emsp;"cfg": **config\_body**,<br> &emsp;&emsp;&emsp;"from": **validator-public-key**,<br> &emsp;&emsp;&emsp;"signature": **validator-node-signature**<br> &emsp;&emsp;},<br> &emsp;"votes\_history\_hash": **vote-history-hash**<br> &emsp;}<br> }|
| `/api/v1/configs/<config-hash>/votes` | GET | Lookup votes for a config propose by config hash<br> If a vote from validator is absent - `null` returned at the corresponding index in json-array | {<br> &emsp;"Votes": [<br> &emsp;&emsp;{<br> &emsp;&emsp;&emsp;"cfg\_hash": **configuration-hash**,<br> &emsp;&emsp;&emsp;"from": **validator-public-key**,<br> &emsp;&emsp;&emsp;"signature": **validator-node-signature**<br> &emsp;&emsp;},<br> &emsp;&emsp;**null**,<br> &emsp;&emsp;...<br> &emsp;]<br> }|

### Private endpoints

Posting a new config can be performed by any validator maintainer via private 
endpoint.

-   it's important to specify `previous_cfg_hash` in new config body, 
    which should be equal to `hash` of a config, actual at the moment 
    when the new propose is being composed.

-   `cfg_hash`, returned in response to `postpropose` request, should be used 
    as `<config_hash-vote-for>` parameter of `postvote` request. 

[Examples](#private-response-samples)

| Endpoint      | HTTP method   | Description | Response template |
| ------------- | ------------- | ------------| ------------------ |
| `/api/v1/configs/postpropose`         | POST | Post proposed config body | {<br> &emsp;"cfg\_hash": **configuration-hash**,<br> &emsp;"tx\_hash": **transaction-hash**<br> }|
| `/api/v1/configs/<config-hash-vote-for>/postvote`      | POST | Vote for a configuration having specific hash | {<br> &emsp;"tx\_hash": **transaction-hash**<br> } |

### Public response samples

1.  `http://127.0.0.1:8000/api/v1/config/actual`

    ```javascript
    {                                                                                            
      "config": {
        "actual_from": 5500,
        "consensus": {
          "peers_timeout": 10000,
          "propose_timeout": 500,
          "round_timeout": 3000,
          "status_timeout": 5000,
          "txs_block_limit": 1000
        },
        "previous_cfg_hash": "daeb250090f2d6b3689a5effd32cb16a77b7770bb1df123e1f32b13143cd3623",
        "services": {
          "1": null
        },
        "validators": [
          "1087206077acf8a456e78cf52fef0f8f275becbb05338dd58822f29015f56f62",
          "44d7e4d9df214a5d946e0f0c955c628f2e08ffedac9eba079446a183715a0796",
          "3484e75181e584787da3a5fe040243b14e1275c9e277ba639e0e2169c5473d9f",
          "d5864b6eb03fd70971d1b25302d2c344cc894d4b42bb953dd17a1d0fe4fba9c5"
        ]
      },
      "hash": "f3e6f3e242365e6d2e1c577461c5924292249f9b52e88b51132a44d1be674e7a"
    }
    ```

1.  `http://127.0.0.1:8000/api/v1/config/following` - format same as 
for `actual`

1.  `http://127.0.0.1:8000/api/v1/
    configs/f3e6f3e242365e6d2e1c577461c5924292249f9b52e88b51132a44d1be674e7a` 

    ```javascript                                                                                                                                         
    {
      "committed_config": {
        "actual_from": 5500,
        "consensus": {
          "peers_timeout": 10000,
          "propose_timeout": 500,
          "round_timeout": 3000,
          "status_timeout": 5000,
          "txs_block_limit": 1000
        },
        "previous_cfg_hash": "daeb250090f2d6b3689a5effd32cb16a77b7770bb1df123e1f32b13143cd3623",
        "services": {
          "1": null
        },
        "validators": [
          "1087206077acf8a456e78cf52fef0f8f275becbb05338dd58822f29015f56f62",
          "44d7e4d9df214a5d946e0f0c955c628f2e08ffedac9eba079446a183715a0796",
          "3484e75181e584787da3a5fe040243b14e1275c9e277ba639e0e2169c5473d9f",
          "d5864b6eb03fd70971d1b25302d2c344cc894d4b42bb953dd17a1d0fe4fba9c5"
        ]
      },
      "propose": {
        "num_votes": "4",
        "tx_propose": {
          "cfg": {
            "actual_from": 5500,
            "consensus": {
              "peers_timeout": 10000,
              "propose_timeout": 500,
              "round_timeout": 3000,
              "status_timeout": 5000,
              "txs_block_limit": 1000
            },
            "previous_cfg_hash": "daeb250090f2d6b3689a5effd32cb16a77b7770bb1df123e1f32b13143cd3623",
            "services": {
              "1": null
            },
            "validators": [
              "1087206077acf8a456e78cf52fef0f8f275becbb05338dd58822f29015f56f62",
              "44d7e4d9df214a5d946e0f0c955c628f2e08ffedac9eba079446a183715a0796",
              "3484e75181e584787da3a5fe040243b14e1275c9e277ba639e0e2169c5473d9f",
              "d5864b6eb03fd70971d1b25302d2c344cc894d4b42bb953dd17a1d0fe4fba9c5"
            ]
          },
          "from": "1087206077acf8a456e78cf52fef0f8f275becbb05338dd58822f29015f56f62",
          "signature": "b949a3131080995179ca547ec128cb5df0bc731d3c2b7737d925ab3aba76b33b279960eddb89f222c301047d6d0e4b797945230fcc05d01378d92e0f7686d705"
        },
        "votes_history_hash": "f8349c1b2f17511c95a18e21e37dfb51348d35f39f5fdfc9191740e4ef479928"
      }
    }
    ```

1.  `http://127.0.0.1:8000/api/v1/configs/
    f3e6f3e242365e6d2e1c577461c5924292249f9b52e88b51132a44d1be674e7a/votes` 

    ```javascript
    {                                                                                                                                                     
      "Votes": [
        {
          "cfg_hash": "f3e6f3e242365e6d2e1c577461c5924292249f9b52e88b51132a44d1be674e7a",
          "from": "1087206077acf8a456e78cf52fef0f8f275becbb05338dd58822f29015f56f62",
          "signature": "e79ae7ea9c12f1a1cdde52b7643cd3b6a8e6a64ea1a5c8bae51aec060c521e33b5a7c0233955a3aa6167243e8e49ff98e104f99bc1eae31cbf198bdd4ca95a02"
        },
        {
          "cfg_hash": "f3e6f3e242365e6d2e1c577461c5924292249f9b52e88b51132a44d1be674e7a",
          "from": "44d7e4d9df214a5d946e0f0c955c628f2e08ffedac9eba079446a183715a0796",
          "signature": "0d82841e448ac073d99ff25c6e62a7e0b832c4cc3da0947b8fcab28c7e53fb925847642af9af4ed3304334d0db38e03f7474b9342ccc7a6bc171358723f3930e"
        },
        {
          "cfg_hash": "f3e6f3e242365e6d2e1c577461c5924292249f9b52e88b51132a44d1be674e7a",
          "from": "3484e75181e584787da3a5fe040243b14e1275c9e277ba639e0e2169c5473d9f",
          "signature": "57366360e10541a59c7f4b692089a1baa3d849083dc7b21a143d00b47392eb512b859bc07aef9748b1720d2273484a605f662b6ad53e3eeb8f4ca89ba06aba05"
        },
        null
      ]
    }
    ```

### Private response samples

1.  `http://127.0.0.1:8010/api/v1/configs/postpropose`

    ```bash
    curl -X POST -d '{                                                                                         
    		  "actual_from": 5500,
    		  "consensus": {
    		    "peers_timeout": 10000,
    		    "propose_timeout": 500,
    		    "round_timeout": 3000,
    		    "status_timeout": 5000,
    		    "txs_block_limit": 1000
    		  },
    		  "previous_cfg_hash": "daeb250090f2d6b3689a5effd32cb16a77b7770bb1df123e1f32b13143cd3623",
    		  "services": {
    		    "1": null
    		  },
    		  "validators": [
    		    "1087206077acf8a456e78cf52fef0f8f275becbb05338dd58822f29015f56f62",
    		    "44d7e4d9df214a5d946e0f0c955c628f2e08ffedac9eba079446a183715a0796",
    		    "3484e75181e584787da3a5fe040243b14e1275c9e277ba639e0e2169c5473d9f",
    		    "d5864b6eb03fd70971d1b25302d2c344cc894d4b42bb953dd17a1d0fe4fba9c5"
    		  ]
    }'  http://127.0.0.1:8010/api/v1/configs/postpropose
    ```
    ```javascript
    {
      "cfg_hash": "f3e6f3e242365e6d2e1c577461c5924292249f9b52e88b51132a44d1be674e7a",
      "tx_hash": "b833617f45e79a785335fdfff161e976f7fa524cd69791fda79344d20f882060"
    }
    ```

1.  `http://127.0.0.1:8012/api/v1/configs/
    f3e6f3e242365e6d2e1c577461c5924292249f9b52e88b51132a44d1be674e7a/postvote`

    ```bash
    curl -X POST -d '{}' http://127.0.0.1:8012/api/v1/configs/f3e6f3e242365e6d2e1c577461c5924292249f9b52e88b51132a44d1be674e7a/postvote
    ```
    ```javascript
    {
      "tx_hash": "3e057783ff4bcb5f180625838d6cfb6317a161ea7024fb35372c1ce722dfc066"
    }
    ```
