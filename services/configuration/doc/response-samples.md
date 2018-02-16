### Public endpoints' response samples

1. <http://127.0.0.1:8000/api/services/configuration/v1/configs/actual>

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

1. <http://127.0.0.1:8000/api/services/configuration/v1/configs/following> -
   format same as for `actual`

1. <http://127.0.0.1:8000/api/services/configuration/v1/configs/d28fd41b85d4aef0c253f00c31d9a8f1a83afd271b3923b5fced6efbecf0bec7>

    ```javascript
    {
      "committed_config": {
        "actual_from": 450,
        "consensus": {
          "peers_timeout": 10000,
          "propose_timeout": 500,
          "round_timeout": 3000,
          "status_timeout": 5000,
          "txs_block_limit": 1000
        },
        "previous_cfg_hash": "267e8cd86262117be995379e7f5acd205cac619ed5d346cc42d5c3578de33c06",
        "services": {
          "1": null
        },
        "validators": [
          "de24ff6ba3ac92035ab09792c397d9af9264528af689cdaed98688a595e5b6ac",
          "5eb97034457d7632cd6f3d3230f958ef2f00c167e514d34b59975277fbb24baa",
          "1d087863b9b474520dd529ac16815d9a45b6c7d13133c7db0f59b94642b4e911",
          "1995c0a3f6313e872549316d52d2f070cc10ecebe78ce3bc730af1d24537f420"
        ]
      },
      "propose": {
        "num_validators": "4",
        "tx_propose": {
          "body": {
            "cfg": "{\"previous_cfg_hash\":\"267e8cd86262117be995379e7f5acd205cac619ed5d346cc42d5c3578de33c06\",\"actual_from\":450,\"validators\":[\"de24ff6ba3ac92035ab09792c397d9af9264528af689cdaed98688a595e5b6ac\",\"5eb97034457d7632cd6f3d3230f958ef2f00c167e514d34b59975277fbb24baa\",\"1d087863b9b474520dd529ac16815d9a45b6c7d13133c7db0f59b94642b4e911\",\"1995c0a3f6313e872549316d52d2f070cc10ecebe78ce3bc730af1d24537f420\"],\"consensus\":{\"round_timeout\":3000,\"status_timeout\":5000,\"peers_timeout\":10000,\"propose_timeout\":500,\"txs_block_limit\":1000},\"services\":{\"1\":null}}",
            "from": "de24ff6ba3ac92035ab09792c397d9af9264528af689cdaed98688a595e5b6ac"
          },
          "message_id": 0,
          "network_id": 0,
          "protocol_version": 0,
          "service_id": 1,
          "signature": "0033beb4d628d0475a34c176eeea192b25dd56ac0652a952ee89ecc858e027532606771378e8a5970c5ed99f5d5968aed6d71e5b6c56d45908b37ff0e736f002"
        },
        "votes_history_hash": "9563a61a7eeef5199aef320547e45aa18ed023f2f0c63b740e0c6c1b93021709"
      }
    }
    ```

1. <http://127.0.0.1:8000/api/services/configuration/v1/configs/d28fd41b85d4aef0c253f00c31d9a8f1a83afd271b3923b5fced6efbecf0bec7/votes>

    ```javascript
    [
        {
          "body": {
            "cfg_hash": "d28fd41b85d4aef0c253f00c31d9a8f1a83afd271b3923b5fced6efbecf0bec7",
            "from": "de24ff6ba3ac92035ab09792c397d9af9264528af689cdaed98688a595e5b6ac"
          },
          "message_id": 1,
          "network_id": 0,
          "protocol_version": 0,
          "service_id": 1,
          "signature": "a5767124e1dc5166a536a44571d8c3f848a9cb8b424aeb913499c1a665f9f051a9d20d92fb3e9c6e8b76fe64f60789de52904b778b8de6458299d2ae3603a00c"
        },
        {
          "body": {
            "cfg_hash": "d28fd41b85d4aef0c253f00c31d9a8f1a83afd271b3923b5fced6efbecf0bec7",
            "from": "5eb97034457d7632cd6f3d3230f958ef2f00c167e514d34b59975277fbb24baa"
          },
          "message_id": 1,
          "network_id": 0,
          "protocol_version": 0,
          "service_id": 1,
          "signature": "4853d9b622b55eff57327425ee83c95b75b8ede0641d0c684747a6ff44e5ffe7e06b0d7b070364e1bae3cbf5bde18c590a2ae9952b65ba51966a4fce913e2f00"
        },
        null,
        {
          "body": {
            "cfg_hash": "d28fd41b85d4aef0c253f00c31d9a8f1a83afd271b3923b5fced6efbecf0bec7",
            "from": "1995c0a3f6313e872549316d52d2f070cc10ecebe78ce3bc730af1d24537f420"
          },
          "message_id": 1,
          "network_id": 0,
          "protocol_version": 0,
          "service_id": 1,
          "signature": "ec041203eebd2b3aa2c5353e98142c94f8b8e40ba76de2595d26cab9ddafd301c26d697cbc5b5f355db5be42fbd2f7309c8f4f3eddd24303691ef07c3d4fab06"
        }
    ]
    ```

1. <http://127.0.0.1:8000/api/services/configuration/v1/configs/committed>

    ```javascript
    [
      {
        "config": {
          "actual_from": 0,
          "consensus": {
            "peers_timeout": 10000,
            "propose_timeout": 500,
            "round_timeout": 3000,
            "status_timeout": 5000,
            "txs_block_limit": 1000
          },
          "previous_cfg_hash": "0000000000000000000000000000000000000000000000000000000000000000",
          "services": {
            "1": null
          },
          "validators": [
            "3f5925a3ce57ed059bc45d6b358dc4d68be5ca39c9a8d649d22458a67560d284"
          ]
        },
        "hash": "fd1234f01cac886f7ec4a236e15958d6a9095a28d145ec80925abffc6a702565"
      },
      {
        "config": {
          "actual_from": 3000,
          "consensus": {
            "peers_timeout": 10000,
            "propose_timeout": 500,
            "round_timeout": 3000,
            "status_timeout": 5000,
            "txs_block_limit": 1000
          },
          "previous_cfg_hash": "fd1234f01cac886f7ec4a236e15958d6a9095a28d145ec80925abffc6a702565",
          "services": {
            "1": {
              "param": "value1"
            }
          },
          "validators": [
            "3f5925a3ce57ed059bc45d6b358dc4d68be5ca39c9a8d649d22458a67560d284"
          ]
        },
        "hash": "64c404b0ae6aa42aafc72398b0f454915902e094fde70029e7b6ab9d4d3bcd68"
      },
      {
        "config": {
          "actual_from": 4000,
          "consensus": {
            "peers_timeout": 10000,
            "propose_timeout": 500,
            "round_timeout": 3000,
            "status_timeout": 5000,
            "txs_block_limit": 1000
          },
          "previous_cfg_hash": "64c404b0ae6aa42aafc72398b0f454915902e094fde70029e7b6ab9d4d3bcd68",
          "services": {
            "1": {
              "param": "value_5"
            }
          },
          "validators": [
            "3f5925a3ce57ed059bc45d6b358dc4d68be5ca39c9a8d649d22458a67560d284"
          ]
        },
        "hash": "4b149012d686b3b66a9a8061e7beb2e72e601675df9fa53e0afde418c0bcb7f4"
      }
    ]
    ```

1. <http://127.0.0.1:8000/api/services/configuration/v1/configs/proposed?previous_cfg_hash=64c404b0ae6aa42aafc72398b0f454915902e094fde70029e7b6ab9d4d3bcd68&actual_from=3100>

    ```javascript
    [
      {
        "hash": "acb7def92c28bc72c2de8b41b648a1e301f1a404cf79d5c350841b27abd30ab2",
        "propose_data": {
          "num_validators": "1",
          "tx_propose": {
            "cfg": {
              "actual_from": 3400,
              "consensus": {
                "peers_timeout": 10000,
                "propose_timeout": 500,
                "round_timeout": 3000,
                "status_timeout": 5000,
                "txs_block_limit": 1000
              },
              "previous_cfg_hash": "64c404b0ae6aa42aafc72398b0f454915902e094fde70029e7b6ab9d4d3bcd68",
              "services": {
                "1": {
                  "param": "value_4"
                }
              },
              "validators": [
                "3f5925a3ce57ed059bc45d6b358dc4d68be5ca39c9a8d649d22458a67560d284"
              ]
            },
            "from": "3f5925a3ce57ed059bc45d6b358dc4d68be5ca39c9a8d649d22458a67560d284",
            "signature": "e7e43c7119cc077d6a9a6e2e3c9948441b48decaf7d9ce06b663b5c974bf12c95e4bc74aa2ace418ea85ad938e5afe3b31338c47ff44c805a04dea547b68b30b"
          },
          "votes_history_hash": "d397c4c800c33521cfc9b4fa8f378ab85018902b42d9bbc2fcdaf27ff88a9dd0"
        }
      },
      {
        "hash": "c9b98af9a860d4bf9479c3b17a22527c8a59acf3b68de43d748a362ce6ec67b1",
        "propose_data": {
          "num_validators": "1",
          "tx_propose": {
            "cfg": {
              "actual_from": 3700,
              "consensus": {
                "peers_timeout": 10000,
                "propose_timeout": 500,
                "round_timeout": 3000,
                "status_timeout": 5000,
                "txs_block_limit": 1000
              },
              "previous_cfg_hash": "64c404b0ae6aa42aafc72398b0f454915902e094fde70029e7b6ab9d4d3bcd68",
              "services": {
                "1": {
                  "param": "value_4"
                }
              },
              "validators": [
                "3f5925a3ce57ed059bc45d6b358dc4d68be5ca39c9a8d649d22458a67560d284"
              ]
            },
            "from": "3f5925a3ce57ed059bc45d6b358dc4d68be5ca39c9a8d649d22458a67560d284",
            "signature": "b82e8c9ad45af8a7abcfce9a277da21b41964e87d07505e9a7552fafc81e90d255c93326dfb07a65b66b81114fee093e504339c3338c85106e4e8821552c990f"
          },
          "votes_history_hash": "d397c4c800c33521cfc9b4fa8f378ab85018902b42d9bbc2fcdaf27ff88a9dd0"
        }
      },
      {
        "hash": "4b149012d686b3b66a9a8061e7beb2e72e601675df9fa53e0afde418c0bcb7f4",
        "propose_data": {
          "num_validators": "1",
          "tx_propose": {
            "cfg": {
              "actual_from": 4000,
              "consensus": {
                "peers_timeout": 10000,
                "propose_timeout": 500,
                "round_timeout": 3000,
                "status_timeout": 5000,
                "txs_block_limit": 1000
              },
              "previous_cfg_hash": "64c404b0ae6aa42aafc72398b0f454915902e094fde70029e7b6ab9d4d3bcd68",
              "services": {
                "1": {
                  "param": "value_5"
                }
              },
              "validators": [
                "3f5925a3ce57ed059bc45d6b358dc4d68be5ca39c9a8d649d22458a67560d284"
              ]
            },
            "from": "3f5925a3ce57ed059bc45d6b358dc4d68be5ca39c9a8d649d22458a67560d284",
            "signature": "83084e0bb334dc5806354c537686c1a04cdf0f86fc23aa9ff950eedca8ed6a4082325686be4a0878fce60deaebe3b5ddc7a13735b1a74f5cf1753210601f6004"
          },
          "votes_history_hash": "4ad431e86676907b228a8bf0ba61f73df0a6392271b30cb977382cfc291c18ee"
        }
      }
    ]
    ```

### Private endpoints' response samples

1. <http://127.0.0.1:8010/api/services/configuration/v1/configs/postpropose>

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

1. <http://127.0.0.1:8012/api/services/configuration/v1/configs/f3e6f3e242365e6d2e1c577461c5924292249f9b52e88b51132a44d1be674e7a/postvote>

    ```bash
    curl -X POST -d '{}' http://127.0.0.1:8012/api/v1/configs/f3e6f3e242365e6d2e1c577461c5924292249f9b52e88b51132a44d1be674e7a/postvote
    ```
    ```javascript
    {
      "tx_hash": "3e057783ff4bcb5f180625838d6cfb6317a161ea7024fb35372c1ce722dfc066"
    }
    ```
