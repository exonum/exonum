#!/bin/bash

#
# Bash script for testing the cryptocurrency demo.
#

set -e

# Base URL for demo service endpoints
BASE_URL=http://127.0.0.1:8080/api/services/cryptocurrency/v1
TRANSACTION_URL=http://127.0.0.1:8080/api/explorer/v1/transactions

# Directory with the script.
ROOT_DIR=`dirname $0`

# Exit status
STATUS=0

# Launches the cryptocurrency demo and waits until it starts listening
# on the TCP port 8080.
function launch-server {
    cargo run -p exonum-cryptocurrency --example demo &
    CTR=0
    MAXCTR=60
    while [[
      ( -z `lsof -iTCP -sTCP:LISTEN -n -P 2>/dev/null | awk '{ if ($9 == "127.0.0.1:8080") { print $2 } }'` )
      && ( $CTR -lt $MAXCTR )
    ]]; do
      sleep 1
      CTR=$(( $CTR + 1 ))
    done

    if [[ $CTR == $MAXCTR ]]; then
        echo "Failed to launch the server; aborting"
        exit 1
    fi
}

# Kills whatever program is listening on the TCP port 8080, on which the cryptocurrency
# demo needs to bind to.
function kill-server {
    SERVER_PID=`lsof -iTCP -sTCP:LISTEN -n -P 2>/dev/null | awk '{ if ($9 == "127.0.0.1:8080") { print $2 } }'`
    if [[ -n $SERVER_PID ]]; then
        # First, try to send the shutdown message to the node in order to shut down it gracefully.
        curl -X POST http://127.0.0.1:8081/api/system/v1/shutdown &>/dev/null
        sleep 1

        SERVER_PID=`lsof -iTCP -sTCP:LISTEN -n -P 2>/dev/null | awk '{ if ($9 == "127.0.0.1:8080") { print $2 } }'`
        if [[ -n $SERVER_PID ]]; then
            kill -KILL $SERVER_PID
        fi
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
      ( `echo $3 | jq ".message == $2"` == "true") \
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
      ( `echo $2 | jq ".message == $1"` == "true" ) \
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
check-transaction abe9ac1e

echo "Creating a wallet for Bob..."
create-wallet "$ROOT_DIR/create-wallet-2.json"
check-transaction 59198cca

sleep 5

echo "Transferring funds from Alice to Bob"
transfer "$ROOT_DIR/transfer-funds.json"
check-transaction b5d68015

echo "Waiting until transactions are committed..."
sleep 3

echo "Retrieving info on all wallets..."
RESP=`curl $BASE_URL/wallets 2>/dev/null`
# Wallet records in the response are deterministically ordered by increasing
# public key. As Alice's pubkey is lexicographically lesser than Bob's, it it possible to
# determine his wallet as .[0] and hers as .[1].
check-request "Alice" 95 "`echo $RESP | jq .[0]`"
check-request "Bob" 105 "`echo $RESP | jq .[1]`"

echo "Retrieving info on Alice's wallet..."
RESP=`curl $BASE_URL/wallet?pub_key=070122b6eb3f63a14b25aacd7a1922c418025e04b1be9d1febdfdbcf67615799 2>/dev/null`
check-request "Alice" 95 "$RESP"

echo "Retrieving Alice's transaction info..."
TXID=abe9ac1eef23b4cda7fc408ce488b233c3446331ac0f8195b7d21a210908b447
RESP=`curl $TRANSACTION_URL?hash=$TXID 2>/dev/null`
EXP=`cat "$ROOT_DIR/create-wallet-1.json" | jq ".tx_body"`
check-create-tx "Alice" "$EXP" "$RESP"

echo "Retrieving transfer transaction info..."
TXID=b5d68015cb47f1b1f909e7667c219f1c63a0b7c978cdd6e8ffc279d05ba66fec
RESP=`curl $TRANSACTION_URL?hash=$TXID 2>/dev/null`
EXP=`cat "$ROOT_DIR/transfer-funds.json" | jq ".tx_body"`
check-transfer-tx "$EXP" "$RESP"

kill-server
exit $STATUS
