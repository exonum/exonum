#!/usr/bin/env python3
from subprocess import Popen, DEVNULL, PIPE, run
import os
import shutil
import re
import argparse
import threading
from time import sleep
from array import *

# there was a problem, described in 127 issue: code like proc.kill() followed by proc.stderr.readlines() trunk captured logs by few dozens of kilobytes.
# in order to deal with this issue, following crutch with separate thread for every popen process and global vars is created

# define flag (which is responsible to note node processes when to finish)
flag = True

# define node_logs var
node_logs = ["", "", "", "", ]

# define node process runner and log collector
def node_proc_runner(node_index, node_args, stdout, stderr, env):
    print("about to run process with args: " + str(node_args))
    node_proc = Popen(node_args, stdout=stdout, stderr=stderr, env=env)

    while flag:
        next_line = str(node_proc.stderr.readline().decode("utf8"))
        node_logs[node_index] += next_line

    print("about to kill process with args: " + str(node_args))
    node_proc.kill()


parser = argparse.ArgumentParser(description='Exonum benchmark util.')
parser.add_argument('--exonum-dir', dest='exonum_dir',
                    action="store", default="/tmp/exonum")
parser.add_argument('--output-file', dest='output_file',
                    action="store", default=os.environ["HOME"] + "/logs/bench.csv")
parser.add_argument('--node-type', dest='node_type',
                    action="store", default="timestamping")
parser.add_argument('--tx-count', dest='tx_count', type=int,
                    action="store", default=200-1)
parser.add_argument('--tx-package-size', dest='tx_package_size', type=int,
                    action="store", default=90)
parser.add_argument('--tx-timeout', dest='tx_timeout', type=int,
                    action="store", default=100)
args = parser.parse_args()

# Config section

print("Config section:")
dest_dir = args.exonum_dir
output_file = args.output_file
node_type = args.node_type

tx_count = args.tx_count
tx_package_size = args.tx_package_size
tx_timeout = args.tx_timeout

# Preparing dest_dir

print("Preparing dest_dir:")
shutil.rmtree(dest_dir + "/db")
os.makedirs(dest_dir + "/db")
shutil.rmtree(dest_dir + "/logs", ignore_errors=True)
os.makedirs(dest_dir + "/logs")

# Helper functions

def is_tx_hash_found_in_node(tx_hash, node_number):
    r = run(["blockchain_utils",
             "-d", dest_dir + "/db/" + str(node_number),
             "find_tx", tx_hash],
            stderr=DEVNULL, stdout=DEVNULL)
    return 0 == r.returncode

# function which search for some tx_hash in all 4 dbs (include node - tx_generator)
def is_tx_hash_found_in_dbs(tx_hash, array_of_unfound_txs):

    number_of_nodes_where_tx_is_found = 0
    for node in range(0, 4):

        if not is_tx_hash_found_in_node(tx_hash, node):
            # return True
            array_of_unfound_txs[node] += 1

    # return False
    return array_of_unfound_txs

def update_data_with_node_log(data, node_log):
    # Analyze node log
    for entry in node_log.splitlines():
        # entry = str(entry.decode("utf8"))
        m = re.search(r"^(\d+).*commited=(\d+).*", entry)
        # print(entry, end='')
        if m is not None:
            # print(entry, end='')
            ts = int(m.group(1))
            cnt = int(m.group(2))
            data.append({"ts": ts, "commited": cnt, "sended": None})
    return data

def get_txs_from_tx_gen_log_and_update_data(tx_gen_log, data):
    # write out tx_generator log
    tx_gen_log_output_file = dest_dir + "/logs/tx_generator.log"
    open(tx_gen_log_output_file, 'w+').write(str(tx_gen_log.decode("utf8")))

    print("get_txs_from_tx_gen_log_and_update_data")
    txs = []
    for entry in str(tx_gen_log.decode("utf8")).splitlines():
        print("entry: " + entry)
        m = re.search(r"^.*(\d+).*count=(\d+).*last_tx_hash=(\w+).*", entry)
        if m is not None:
            print(entry)
            ts = int(m.group(1))
            cnt = int(m.group(2))
            tx_hash = str(m.group(3))
            data.append({"ts": ts, "commited": None, "sended": cnt, "tx_hash": tx_hash})
            txs.append(tx_hash)
    return txs

def print_output_to_bench_file(node_number, node_log, tx_gen_log):
    # output_file = dest_dir + "/logs/bench" + str(node_number) + ".csv"
    bench_output_file = dest_dir + "/logs/bench" + str(node_number) + ".csv"
    node_log_output_file = dest_dir + "/logs/node_" + str(node_number) + ".log"
    open(node_log_output_file, 'w+').write(str(node_log))
    data = []
    update_data_with_node_log(data, node_log)
    txs = get_txs_from_tx_gen_log_and_update_data(tx_gen_log, data)

    with open(bench_output_file, 'w+') as out:
        out.write("number of lines in node_log: {}\n".format(str(len(node_log))))
        current_block_size = 0
        last_committed = 0
        commited = 0
        sended = 0
        out.write("timestamp (ms),sended,commited,current_block_size,tx_hash,is_tx_hash_found_in_node\n")
        data.sort(key=lambda x: x['ts'])
        for value in data:
            if value["sended"] is not None:
                sended = value["sended"]
            # print("!!!:" + str(value))
            # print("!!!:" + str(value["tx_hash"]))
            if "tx_hash" in value.keys() and value["tx_hash"] is not None:
                # print("!!!:" + str(value["tx_hash"]))
                tx_hash = value["tx_hash"]
                is_tx_hash_found_in_node_var = is_tx_hash_found_in_node(tx_hash, node_number)
            else:
                tx_hash = "None"
                is_tx_hash_found_in_node_var = "None"
            if value["commited"] is not None:
                commited = value["commited"]
                current_block_size = commited - last_committed
                last_committed = commited
            else:
                current_block_size = None
                commited = None
            out.write("{},{},{},{},{},{}\n".format(value["ts"], sended, commited, current_block_size, tx_hash, is_tx_hash_found_in_node_var))

        out.write("len(txs): {}\n".format(str(len(txs))))
        out.write("array_of_unfound_txs: {}\n".format(str(array_of_unfound_txs)))

# ide aof the function is to produce args to run node with index i
def node_args(i):
    return [
        node_type,
        "run",
        "--node-config",        dest_dir + "/validators/%s.toml" % str(i),
        "--leveldb-path",       dest_dir + "/db/%s" % str(i),
    ]

# idea of the function is to return args to start process for node with index i
def node_proc_args(i):
    return (
        i,
        node_args(i),
        PIPE, PIPE,
        dict(node_env, RUST_LOG="exonum=trace")
    )

# Running nodes
print("Running nodes:")

node_env = os.environ.copy()
node_env["RUST_BACKTRACE"] = "1"

node_args_ = [
    node_type,
    "run"
]

tx_gen_args = [
    "tx_generator",
    "run",
    "--node-config",        dest_dir + "/validators/3.toml",
    "--leveldb-path",       dest_dir + "/db/3",
    "--tx-package-size",    str(tx_package_size),
    "--tx-timeout",         str(tx_timeout),
                         str(tx_count),  # COUNT - number of all txs
          ]

# procs = [
#     Popen(node_args + ["-d", dest_dir + "/db/0", "0"],
#           stdout=PIPE, stderr=PIPE,
#           env=dict(node_env, RUST_LOG="exonum=trace")),
#     Popen(node_args + ["-d", dest_dir + "/db/1", "1"],
#           stdout=PIPE, stderr=PIPE,
#           env=dict(node_env, RUST_LOG="exonum=trace")),
#     Popen(node_args + ["-d", dest_dir + "/db/2", "2"],
#           stdout=PIPE, stderr=PIPE,
#           env=dict(node_env, RUST_LOG="exonum=trace")),
# ]

# t0 = threading.Thread(target=node_proc_runner, args=(0, node_args + ["-d", dest_dir + "/db/0", "0"],
#                                                      PIPE, PIPE,
#                                                      dict(node_env, RUST_LOG="exonum=trace")))
# t1 = threading.Thread(target=node_proc_runner, args=(1, node_args + ["-d", dest_dir + "/db/1", "1"],
#                                                      PIPE, PIPE,
#                                                      dict(node_env, RUST_LOG="exonum=trace")))
# t2 = threading.Thread(target=node_proc_runner, args=(2, node_args + ["-d", dest_dir + "/db/2", "2"],
#                                                      PIPE, PIPE,
#                                                      dict(node_env, RUST_LOG="exonum=trace")))

tx_gen_proc = Popen(tx_gen_args,
                    stdout=DEVNULL, stderr=PIPE,
                    env=dict(node_env, RUST_LOG="tx_generator=trace,exonum=trace")
                    )

# start threads
print("Running node with args: " + str(node_args(0)))
# t0.start()
threading.Thread(target=node_proc_runner, args=(node_proc_args(0))).start()
print("Running node with args: " + str(node_args(1)))
# t1.start()
threading.Thread(target=node_proc_runner, args=(node_proc_args(1))).start()
print("Running node with args: " + str(node_args(2)))
# t2.start()
threading.Thread(target=node_proc_runner, args=(node_proc_args(2))).start()

print("Running tx_gen_proc with params: " + str(tx_gen_args))
(_, tx_gen_log) = tx_gen_proc.communicate()
# try:
#     (_, tx_gen_log) = tx_gen_proc.communicate(timeout = 1200)
# except:
#     tx_gen_proc.kill()
#     (_, tx_gen_log) = tx_gen_proc.communicate()

print("waiting for nodes to catch each other")
sleep(10)

print("killing nodes:")
flag = False
print("waiting for processes to be finished")
sleep(1)


print("collecting logs:")
node_0_log = node_logs[0]
node_1_log = node_logs[1]
node_2_log = node_logs[2]

data = []

# validate transactions
print("validate transactions")
txs = get_txs_from_tx_gen_log_and_update_data(tx_gen_log, data)
print("len(txs):")
print(len(txs))
# assert len(txs) > 0
failed_txs = 0
array_of_unfound_txs = array('l', [0, 0, 0, 0])
for tx in txs:
    array_of_unfound_txs = is_tx_hash_found_in_dbs(tx, array_of_unfound_txs)

print("array_of_unfound_txs: " + str(array_of_unfound_txs))

# Print csv output
print_output_to_bench_file(0, node_0_log, tx_gen_log)
print_output_to_bench_file(1, node_1_log, tx_gen_log)
print_output_to_bench_file(2, node_2_log, tx_gen_log)
# print_output_to_file(3, tx_gen_node_log, tx_gen_log)
# with open(output_file, 'w+') as out:
#     current_block_size = 0
#     last_committed = 0
#     commited = 0
#     sended = 0
#     out.write("timestamp (ms),sended,commited,current_block_size\n")
#     data.sort(key=lambda x: x['ts'])
#     for value in data:
#         if value["sended"] is not None:
#             sended = value["sended"]
#         if value["commited"] is not None:
#             commited = value["commited"]
#             current_block_size = commited - last_committed
#             last_committed = commited
#         out.write("{},{},{},{}\n".format(value["ts"], sended, commited, current_block_size))
#
#     out.write("len(txs): {}\n".format(str(len(txs))))
#     out.write("array_of_unfound_txs: {}\n".format(str(array_of_unfound_txs)))
