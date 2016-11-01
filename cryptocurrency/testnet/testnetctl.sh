#!/bin/bash

node_executable=cryptocurrency
destdir=$2
scriptdir=`dirname "$BASH_SOURCE"`
supervisor_conf=${destdir}/etc/supervisord.conf
sock_file=/tmp/

start() {
    mkdir -p ${destdir}/log/supervisor
    mkdir ${destdir}/run
    mkdir ${destdir}/db
    cp -R ${scriptdir}/etc ${destdir}
    $node_executable -c ${destdir}/testnet.conf generate 9 -p 9000
    cd $destdir
    supervisord
    supervisorctl start leveldb:*
}

stop() {
    cd ${destdir}
    if [ -e /tmp/supervisor_tst_cryptocurrency.sock ]
    then
        supervisorctl shutdown
    fi 
    rm -rf "./db"
    rm -rf "./etc"
}

case "$1" in
    stop)
        stop
        ;;
    start) 
        start
        ;;
esac