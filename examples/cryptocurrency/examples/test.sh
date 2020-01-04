#!/bin/bash

#
# Bash script for testing the cryptocurrency demo.
#

set -e

# Base URL for demo service endpoints
BASE_URL=http://127.0.0.1:8000/api/services/cryptocurrency/v1
TRANSACTION_URL=http://127.0.0.1:8000/api/explorer/v1/transactions

# Directory with the script.
ROOT_DIR=`dirname $0`

# Exit status
STATUS=0

# Launches the cryptocurrency demo and waits until it starts listening
# on the TCP port 8000.
function launch-server {
    cargo run -p exonum-cryptocurrency --example demo &
    CTR=0
    MAXCTR=60
    while [[ ( -z `lsof -iTCP -sTCP:LISTEN -n -P 2>/dev/null |  awk '{ if ($9 == "*:8000") { print $2 } }'` ) && ( $CTR -lt $MAXCTR ) ]]; do
      sleep 1
      CTR=$(( $CTR + 1 ))
    done
    if [[ $CTR == $MAXCTR ]]; then
        echo "Failed to launch the server; aborting"
        exit 1
    fi
}

# Kills whatever program is listening on the TCP port 8000, on which the cryptocurrency
# demo needs to bind to.
function kill-server {
    SERVER_PID=`lsof -iTCP -sTCP:LISTEN -n -P 2>/dev/null |  awk '{ if ($9 == "*:8000") { print $2 } }'`
    if [[ -n $SERVER_PID ]]; then
        kill -9 $SERVER_PID
    fi
}

# Creates a wallet in the cryptocurrency demo.
#
# Arguments:
# - $1: filename with the transaction data
function create-wallet {
    RESP=`curl -H "Content-Type: application/json" -X POST -d @$1 $TRANSACTION_URL 2>/dev/null`
}

# Performs a transfer in the cryptocurrency demo.
#
# Arguments:
# - $1: filename with the transaction data
function transfer {
    RESP=`curl -H "Content-Type: application/json" -X POST -d @$1 $TRANSACTION_URL 2>/dev/null`
}

# Checks a response to an Exonum transaction.
#
# Arguments:
# - $1: expected start of the transaction hash returned by the server
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
# - $1: expected user name
# - $2: expected user balance
# - $3: response JSON that encodes user's wallet information
function check-request {
    if [[ ( `echo $3 | jq .name` == "\"$1\"" ) && ( `echo $3 | jq .balance` == $2 ) ]]; then
        echo "OK, got expected transaction balance $2 for user $1"
    else
        # $RESP here is intentional; we want to output the entire incorrect response
        echo "Unexpected response: $RESP"
        STATUS=1
    fi
}

# Checks a `TxCreateWallet` transaction in the blockchain explorer.
#
# Arguments:
# - $1: expected user name
# - $2: expected transaction in HEX
# - $3: response JSON
function check-create-tx {
    if [[ \
      ( `echo $3 | jq .type` == \"committed\" ) && \
      ( `echo $3 | jq ".content == $2"` == "true") \
    ]]; then
        echo "OK, got expected TxCreateWallet for user $1"
    else
        echo "Unexpected response: $3"
        STATUS=1
    fi
}

# Checks a `TxCreateWallet` transaction in the blockchain explorer.
#
# Arguments:
# - $1: expected transaction in HEX
# - $2: response JSON
function check-transfer-tx {
    if [[ \
      ( `echo $2 | jq .type` == \"committed\" ) && \
      ( `echo $2 | jq ".content == $1"` == "true" ) \
    ]]; then
        echo "OK, got expected TxTransfer between wallets"
    else
        echo "Unexpected response: $2"
        STATUS=1
    fi
}

kill-server
launch-server

echo "Creating a wallet for Alice..."
create-wallet "$ROOT_DIR/create-wallet-1.json"
check-transaction de7283a8

echo "Creating a wallet for Bob..."
create-wallet "$ROOT_DIR/create-wallet-2.json"
check-transaction 34f33f36

sleep 5

echo "Transferring funds from Alice to Bob"
transfer "$ROOT_DIR/transfer-funds.json"
check-transaction 60750247

echo "Waiting until transactions are committed..."
sleep 3

echo "Retrieving info on all wallets..."
RESP=`curl $BASE_URL/wallets 2>/dev/null`
# Wallet records in the response are deterministically ordered by increasing
# public key. As Alice's pubkey is lexicographically lesser than Bob's, it it possible to
# determine his wallet as .[0] and hers as .[1].
check-request "Bob" 105 "`echo $RESP | jq .[0]`"
check-request "Alice" 95 "`echo $RESP | jq .[1]`"

echo "Retrieving info on Alice's wallet..."
RESP=`curl $BASE_URL/wallet?pub_key=763cd266f3f6b6d5746f67477ed39c74c7249991ebbe34446d176fc81b36a41e 2>/dev/null`
check-request "Alice" 95 "$RESP"

echo "Retrieving Alice's transaction info..."
TXID=de7283a8c2a49c476ec91681e795181d9846a5bbce6488d4313a8300b34b4d48
RESP=`curl $TRANSACTION_URL?hash=$TXID 2>/dev/null`
EXP=`cat "$ROOT_DIR/create-wallet-1.json" | jq ".tx_body"`
check-create-tx "Alice" "$EXP" "$RESP"

echo "Retrieving transfer transaction info..."
TXID=6075024770778476b80d4fe880c408f3df4c3df04bff6d2ae81ae1e415449840
RESP=`curl $TRANSACTION_URL?hash=$TXID 2>/dev/null`
EXP=`cat "$ROOT_DIR/transfer-funds.json" | jq ".tx_body"`
check-transfer-tx "$EXP" "$RESP"

kill-server
exit $STATUS
