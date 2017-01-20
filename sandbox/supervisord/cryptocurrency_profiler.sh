#!/bin/bash

testnet_dir=/tmp/exonum_profiler


scriptdir=`pwd`
supervisor_conf=${scriptdir}/sandbox/supervisord/etc/supervisord.conf
export PATH=${scriptdir}/target/release:$PATH
export PATH=${scriptdir}/target/release/examples:$PATH
export SCRIPTS_PATH=${scriptdir}/sandbox/supervisord

run() {

    test -e ${testnet_dir}/supervisord.sock && exit 1


    export TESTNET_DESTDIR="${testnet_dir}/run"
    export PROFILE_STOP_AT_HEIGHT=$2
    #stub generator wallet count
    export TESTNET_COUNT=0

    load $1 &&

    supervisord -c ${supervisor_conf}  &&
    #start validators && tx_Generator
    supervisorctl -c ${supervisor_conf} start cryptocurrency_profiler:* &&

    #wait for report
    while [ ! -e $TESTNET_DESTDIR/profile/flame-00.html ]; do sleep 1; done
    sleep 30 # give a chance to write full report
    mkdir ${scriptdir}/report
    cp -f $TESTNET_DESTDIR/profile/flame-00.html ${scriptdir}/report/
    supervisorctl -c ${supervisor_conf} stop cryptocurrency_profiler:*
    supervisorctl -c ${supervisor_conf} shutdown
}

generate() {
    [[ -z  $1 ]] && echo 'Write wallets count' && exit 1
    export TESTNET_COUNT=$1
    export TESTNET_DESTDIR="${testnet_dir}/generate"
    echo "Generating database in folder $TESTNET_DESTDIR"

    #cargo build --manifest-path sandbox/Cargo.toml --example=tx_generator --release 
    cargo build --manifest-path cryptocurrency/Cargo.toml --features="flame_profile" --release &&    
    #generate config
    mkdir -p $TESTNET_DESTDIR &&
    cryptocurrency generate 4 -p 7320 -o $TESTNET_DESTDIR &&
    mkdir -p $TESTNET_DESTDIR/log/supervisor &&
    mkdir $TESTNET_DESTDIR/run &&
    mkdir $TESTNET_DESTDIR/db &&
    supervisord -c ${supervisor_conf}  &&
    mkdir -p $TESTNET_DESTDIR/profile &&
    #start validators && tx_Generator
    supervisorctl -c ${supervisor_conf} start cryptocurrency_profiler_generate:* &&
    #wait for supervisord death
    while [ -e $TESTNET_DESTDIR/supervisor.sock ]; do sleep 1; done || echo "Run clean first"
}

save() {
    echo "Saving database into ${testnet_dir}/$1"
    cp -r "${testnet_dir}/generate" "${testnet_dir}/$1"
}

load() {
    echo "Loading database ${testnet_dir}/$1"
    echo "Cleanup old running results"
    rm -r ${testnet_dir}/run > /dev/null
    echo "Deploy"
    cp -r "${testnet_dir}/$1" "${testnet_dir}/run"
}

clean() {
    echo "Cleanup ${testnet_dir}"
    echo "Shutdown running tests"
    export TESTNET_DESTDIR="${testnet_dir}/run"
    supervisorctl -c ${supervisor_conf} shutdown > /dev/null

    echo "Shutdown generator"
    export TESTNET_DESTDIR="${testnet_dir}/generate"
    supervisorctl -c ${supervisor_conf} shutdown > /dev/null
    
    rm -r "${testnet_dir}"
}

case "$1" in
    run) 
        run $2 $3
        ;;
    save) 
        save $2
        ;;
    load) 
        load $2
        ;;
    generate) 
        generate $2
        ;;
    clean) 
        clean 
        ;;
esac
