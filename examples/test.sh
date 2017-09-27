#!/bin/bash

set -e

# Exit status
STATUS=0

function launch-server {
    cd ..
    cargo run &
    CTR=0
    MAXCTR=60
    while [[ ( -z `netstat -tlp 2>/dev/null | awk '{ if ($4 == "*:8000") { split($7, pid, /\//); print pid[1] } }'` ) && ( $CTR -lt $MAXCTR ) ]]; do
      sleep 1
      CTR=$(( $CTR + 1 ))
    done
    if [[ $CTR == $MAXCTR ]]; then
        echo "Failed to launch the server; aborting"
        exit 1
    fi
    cd examples
}

function kill-server {
    netstat -tlp 2>/dev/null | awk '{ if ($4 == "*:8000") { split($7, pid, /\//); print pid[1] } }' | xargs -r kill -KILL
}

function send-transaction {
    RESP=`curl -H "Content-Type: application/json" -X POST -d @$1 http://127.0.0.1:8000/api/services/cryptocurrency/v1/wallets/transaction 2>/dev/null`
}

function check-transaction {
    if [[ `echo $RESP | jq .tx_hash` =~ ^\"$1 ]]; then
        echo "OK, got expected transaction hash $1"
    else
        echo "Unexpected response: $RESP"
        STATUS=1
    fi
}

function check-request {
    if [[ ( `echo $3 | jq .name` == "\"$1\"" ) && ( `echo $3 | jq .balance` == "\"$2\"" ) ]]; then
        echo "OK, got expected transaction balance $2 for user $1"
    else
        # $RESP here is intentional; we want to output the entire incorrect response
        echo "Unexpected response: $RESP"
        STATUS=1
    fi
}

kill-server
launch-server

echo "Creating a wallet for Johnny..."
send-transaction create-wallet-1.json
check-transaction 44c6c2c5

echo "Creating a wallet for Janie..."
send-transaction create-wallet-2.json
check-transaction 8714e906

echo "Transferring funds from Johnny to Janie"
send-transaction transfer-funds.json
check-transaction e63b28ca

echo "Waiting until transactions are committed..."
sleep 7

echo "Retrieving info on all wallets..."
RESP=`curl http://127.0.0.1:8000/api/services/cryptocurrency/v1/wallets 2>/dev/null`
check-request "Johnny Doe" 90 "`echo $RESP | jq .[0]`"
check-request "Janie Roe" 110 "`echo $RESP | jq .[1]`"

echo "Retrieving info on Johnny's wallet..."
RESP=`curl http://127.0.0.1:8000/api/services/cryptocurrency/v1/wallet/03e657ae71e51be60a45b4bd20bcf79ff52f0c037ae6da0540a0e0066132b472 2>/dev/null`
check-request "Johnny Doe" 90 "$RESP"

kill-server
exit $STATUS
