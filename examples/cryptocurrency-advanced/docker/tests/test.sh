#!/bin/bash

#
# Bash script for testing the docker container for exonum-cryptocurrency-advanced demo.
#

set -e

# Base URL for demo service endpoints
BASE_URL=http://127.0.0.1:8000/api/services/cryptocurrency/v1
TX_URL=http://127.0.0.1:8000/api/explorer/v1/transactions

# Exit status
STATUS=0

# Runs docker container.
function launch-server {
    docker run -p 8000-8008:8000-8008 exonumhub/exonum-cryptocurrency-advanced:demo & sleep 20
}

function kill-server {
    docker ps | grep exonumhub/exonum-cryptocurrency-advanced:demo | gawk '{print $1}' | xargs docker stop || true
}

# Creates a wallet in the cryptocurrency-advanced demo.
#
# Arguments:
# - $1: filename with the transaction data.
function transaction {
    RESP=`curl -H "Content-Type: application/json" -X POST -d @$1 $TX_URL 2>/dev/null`
    sleep 1
}

# Checks a response to an Exonum transaction.
#
# Arguments:
# - $1: expected start of the transaction hash returned by the server.
function check-transaction {
    if [[ `echo $RESP | jq .tx_hash` =~ ^\"$1 ]]; then
        echo "OK, got expected transaction hash $1"
    else
        echo "Unexpected response: $RESP"
        STATUS=1
    fi
}

# Checks a response to a read request.
#
# Arguments:
# - $1: expected user name.
# - $2: expected user balance.
# - $3: response JSON that encodes user's wallet information.
function check-request {
    if [[ ( `echo $3 | jq '.wallet_proof .to_wallet .entries [0] .value .name'` == "\"$1\"" ) && ( `echo $3 | jq '.wallet_proof .to_wallet .entries [0] .value .balance'` == $2 ) ]]; then
        echo "OK, got expected transaction balance $2 for user $1"
    else
        # $RESP here is intentional; we want to output the entire incorrect response
        echo "Unexpected response: $RESP"
        STATUS=1
    fi
}

# Checks a `CreateWallet` transaction in the blockchain explorer.
#
# Arguments:
# - $1: expected user name
# - $2: expected transaction JSON
# - $3: response JSON
function check-create-tx {
    if [[ \
      ( `echo $3 | jq .type` == \"committed\" ) && \
      ( `echo $3 | jq '.content .debug .name'` == "\"$1\"" ) && \
      ( `echo $3 | jq ".content .message == $2"` == "true" ) \
    ]]; then
        echo "OK, got expected TxCreateWallet for user $1"
    else
        echo "Unexpected response: $3"
        STATUS=1
    fi
}

# Checks a `Transfer` transaction in the blockchain explorer.
#
# Arguments:
# - $1: expected transaction JSON
# - $2: response JSON
function check-transfer-tx {
    if [[ \
      ( `echo $2 | jq .type` == \"committed\" ) && \
      ( `echo $2 | jq ".content .message == $1"` == "true" ) \
    ]]; then
        echo "OK, got expected TxTransfer between wallets"
    else
        echo "Unexpected response: $2"
        STATUS=1
    fi
}

launch-server

echo "Creating a wallet for Alice..."
transaction tx-create-wallet-1.json
check-transaction be8531a8

echo "Creating a wallet for Bob..."
transaction tx-create-wallet-2.json
check-transaction 4d73a036

echo "Add funds to Alice's wallet..."
transaction tx-issue.json
check-transaction 4622eb1e

echo "Transferring funds from Alice to Bob..."
transaction tx-transfer.json
check-transaction 06aa2c6f

echo "Waiting until transactions are committed..."
sleep 5

echo "Retrieving info on Alice's wallet..."
RESP=`curl $BASE_URL/wallets/info?pub_key=cf6e0ddfe440ad799bb78ce2e9b99d60481c2aa0ca7bf968383e1b97981a255d 2>/dev/null`
check-request "Alice" 140 "$RESP"

echo "Retrieving info on Bob's wallet..."
RESP=`curl $BASE_URL/wallets/info?pub_key=e73805c9f0cc566c0ecd61c56d2ac3b25187eec1e2922f23152b5f0c05af8531 2>/dev/null`
check-request "Bob" 110 "$RESP"

echo "Retrieving Alice's transaction info..."
TXID=be8531a869881a8ebfbd202f91028715123089b54082de813cbad3b9485a6c54
RESP=`curl $TX_URL?hash=$TXID 2>/dev/null`
EXP=`cat tx-create-wallet-1.json | jq .tx_body`
check-create-tx "Alice" "$EXP" "$RESP"

echo "Retrieving Bob's transaction info..."
TXID=4d73a036d035387fdb81598c2deed99c36636a29e7dcee23720a7eda38adc08e
RESP=`curl $TX_URL?hash=$TXID 2>/dev/null`
EXP=`cat tx-create-wallet-2.json | jq .tx_body`
check-create-tx "Bob" "$EXP" "$RESP"

echo "Retrieving transfer transaction info..."
TXID=06aa2c6fdb0c1ff06211797d70a01565356be29c36ad6a906382032ca8e889fb
RESP=`curl $TX_URL?hash=$TXID 2>/dev/null`
EXP=`cat tx-transfer.json | jq .tx_body`
check-transfer-tx "$EXP" "$RESP"

kill-server

exit $STATUS
