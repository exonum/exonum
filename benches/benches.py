
# idea of the file is to run a number of benchmarks with different flow of transactions and write out how many transactions were found and keep logs

from subprocess import Popen, DEVNULL, PIPE, run
import os
import shutil
import argparse
from time import sleep

# parse arguments
parser = argparse.ArgumentParser(description='Exonum benchmarks util.')
parser.add_argument('--exonum-dir', dest='exonum_dir',
                    action="store", default="/tmp/exonum")
parser.add_argument('--benches-dir', dest='benches_dir',
                    action="store", default="/tmp/exonum/benches")
parser.add_argument('--tx-package-count', dest='tx_package_count', type=int,
                    action="store", default=20)
parser.add_argument('--tx-package-size-min', dest='tx_package_size_min', type=int,
                    action="store", default=100)
parser.add_argument('--tx-package-size-max', dest='tx_package_size_max', type=int,
                    action="store", default=900)
parser.add_argument('--tx-package-size-step', dest='tx_package_size_step', type=int,
                    action="store", default=600)
parser.add_argument('--tx-timeout', dest='tx_timeout', type=int,
                    action="store", default=100)
args = parser.parse_args()

def print_args():
    # print arguments:
    print("exonum_dir: " + str(args.exonum_dir))
    print("benches_dir: " + str(args.benches_dir))
    print("tx_package_count: " + str(args.tx_package_count))
    print("tx_package_size_min: " + str(args.tx_package_size_min))
    print("tx_package_size_max: " + str(args.tx_package_size_max))
    print("tx_package_size_step: " + str(args.tx_package_size_step))
    print("tx_timeout: " + str(args.tx_timeout))

#     idea of the function is to
#  - create name of directory for current bench (using arguments)
def create_name_of_bench_dir_2(**kwargs):
    bench_dir_name = "bench__"
    # use code from http://stackoverflow.com/a/30418498
    for key,value in kwargs.items():
        bench_dir_name += "%s-%s__" % (key, value)

    return bench_dir_name




# idea of the function is
#  - to run bench with certain arguments (tx_number, tx_package_size and tx_timeout)
def run_bench(exonum_dir, tx_count, tx_package_size, tx_timeout):
    bench_args = [
        "python3",  "./benches/transactions.py",
        "--exonum-dir", str(exonum_dir),
        "--tx-count", str(tx_count),
        "--tx-package-size", str(tx_package_size),
        "--tx-timeout", str(tx_timeout),
    ]
    print("run bench with args: " + str(bench_args))

    bench_proc = Popen(bench_args,
                       stdout=DEVNULL, stderr=PIPE,
                       # env=dict(node_env, RUST_LOG="tx_generator=trace")
                       )

    print("Running tx_gen_proc:")
    (_, bench_log) = bench_proc.communicate()

#     idea of the function is to get information about array_of_unfound_txs
# (from the last line of file bench0.csv from bench older)
# assumptions:
#  - bench folder contains this bench0.csv file
#  - this file contains line with 'array_of_unfound_txs' text (probably, this is a last line of the file)
def get_count_of_unfound_txs(bench_folder_path):
    with open(bench_folder_path + "/bench0.csv") as f:
        for line in f:
            if "array_of_unfound_txs" in line:
                return line

# idea of the function is to copy results from folder /tmp/exonum/logs to folder .../branches/current_branch
def keep_results(logs_dir_path, bench_dir_path):
    shutil.rmtree(bench_dir_path, ignore_errors=True)
    shutil.copytree(logs_dir_path, bench_dir_path)

# idea of the function is
#  - to run bench with certain arguments (tx_number, tx_package_size and tx_timeout)
#  - copy log files and according csv's to user dir (in folder with name which reflects params)
def process_bench(exonum_dir, benches_dir_path, tx_count, tx_package_size, tx_timeout):
    run_bench(exonum_dir, tx_count, tx_package_size, tx_timeout)
    bench_dir_name = create_name_of_bench_dir_2(tx_count=tx_count, tx_package_size=tx_package_size, tx_timeout=tx_timeout)
    bench_dir_path = benches_dir_path + "/" + bench_dir_name
    keep_results(exonum_dir + "/logs", bench_dir_path)
#     print count_of_unfound_txs
    print(bench_dir_name + " - " + get_count_of_unfound_txs(bench_dir_path))
    bench_output_file_path = benches_dir_path + "/count_of_unfound_txs"
    with open(bench_output_file_path, "a") as myfile:
        myfile.write(bench_dir_name + " - " + get_count_of_unfound_txs(bench_dir_path))


print_args()

# clean benches dir
shutil.rmtree(args.benches_dir, ignore_errors=True)
os.makedirs(args.benches_dir)

# loop through the range of flows
package_size_current = args.tx_package_size_min
while package_size_current <= args.tx_package_size_max:
    process_bench(args.exonum_dir, args.benches_dir, package_size_current * args.tx_package_count, package_size_current, args.tx_timeout)
    package_size_current += args.tx_package_size_step
    sleep(10)

