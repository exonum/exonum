### Public response samples

1.  <http://127.0.0.1:8000/api/v1/configs/actual>

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

1.  <http://127.0.0.1:8000/api/v1/configs/following> - format same as 
for `actual`

1.  <http://127.0.0.1:8000/api/v1/configs/f3e6f3e242365e6d2e1c577461c5924292249f9b52e88b51132a44d1be674e7a> 

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

1.  <http://127.0.0.1:8000/api/v1/configs/f3e6f3e242365e6d2e1c577461c5924292249f9b52e88b51132a44d1be674e7a/votes> 

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

1.  <http://127.0.0.1:8010/api/v1/configs/postpropose>

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

1.  <http://127.0.0.1:8012/api/v1/configs/f3e6f3e242365e6d2e1c577461c5924292249f9b52e88b51132a44d1be674e7a/postvote>

    ```bash
    curl -X POST -d '{}' http://127.0.0.1:8012/api/v1/configs/f3e6f3e242365e6d2e1c577461c5924292249f9b52e88b51132a44d1be674e7a/postvote
    ```
    ```javascript
    {
      "tx_hash": "3e057783ff4bcb5f180625838d6cfb6317a161ea7024fb35372c1ce722dfc066"
    }
    ```
