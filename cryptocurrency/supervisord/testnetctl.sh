#!/bin/bash

node_executable=cryptocurrency
destdir=$2
scriptdir=`dirname "$BASH_SOURCE"`
supervisor_conf=${destdir}/etc/supervisord.conf
sock_file=/tmp/

start() {
    ${scriptdir}/create_testnet.sh ${destdir} > /dev/null 2>&1
    cd $destdir
    supervisord
    supervisorctl start leveldb:*
}

stop() {
    cd ${destdir}
    if [ -e /tmp/supervisor_cryptocurrency.sock ]
    then
        supervisorctl stop all
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