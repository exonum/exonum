#!/bin/bash

destdir=$2

scriptdir=`dirname "$BASH_SOURCE"`
supervisor_conf=${destdir}/etc/supervisord.conf
sock_file=/tmp/

enable() {
    mkdir -p ${destdir}/log/supervisor
    mkdir ${destdir}/run
    mkdir ${destdir}/db
    cp -R ${scriptdir}/supervisord/etc ${destdir} || exit 1

    cd ${destdir}
    if [ -e /tmp/supervisord.sock ]
    then
        supervisorctl reload || exit 1
    else
        supervisord || exit 1
    fi 
}

disable() {
    cd ${destdir}
    if [ -e /tmp/supervisord.sock ]
    then
        supervisorctl shutdown || exit 1
    fi 
}

clear() {
    cd ${destdir}
    if [ -e /tmp/supervisord.sock ]
    then
        supervisorctl shutdown || exit 1
    fi 
    rm -rf ${destdir}
}

start() {
    echo $1
    echo ${destdir}
    test -e /tmp/supervisord.sock || exit 1

    template=$1
    cd ${destdir}
    supervisorctl start ${template}:* || exit 1
}

restart() {
    echo $1
    echo ${destdir}
    test -e /tmp/supervisord.sock || exit 1

    template=$1
    cd ${destdir}
    supervisorctl restart ${template}:* || exit 1
}

stop() {
    test -e /tmp/supervisord.sock || exit 1

    template=$1
    cd ${destdir}
    supervisorctl stop ${template}:* || exit 1
}

echo $3
case "$1" in
    start)
        start $3
        ;;
    stop) 
        stop $3
        ;;
    restart)
        restart $3
        ;;
    enable) 
        enable $2
        ;;
    disable) 
        disable $2
        ;;
    clear) 
        clear $2
        ;;
esac