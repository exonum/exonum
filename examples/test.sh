#!/bin/bash

function send-transaction {
    curl -H "Content-Type: application/json" -X POST -d @$1 http://127.0.0.1:8000/api/services/cryptocurrency/v1/wallets/transaction
}

send-transaction create-wallet-1.json
send-transaction create-wallet-2.json
send-transaction transfer-funds.json
