#!/bin/bash

function print_help {
    echo "Usage:
        --all           Display info for all wallets
        --key {key}     Display info for wallet with key: {key}"
}

if [[ $1 == "--all" ]];
then
    curl http://127.0.0.1:8000/api/services/cryptocurrency/v1/wallets
elif [[ $1 == "--key" ]];
then
    curl http://127.0.0.1:8000/api/services/cryptocurrency/v1/wallet/$2
else
    print_help
    exit
fi
