
# idea of the file is to run a number of benchmarks with different flow of transactions and write out how many transactions were found and keep logs

from subprocess import Popen, DEVNULL, PIPE, run
import os
import shutil
import re
import argparse
import fileinput
import sys
from time import sleep

# constants
csv_delimiter = ","
count_of_unfound_txs_file_name = "benches_results.csv"

# parse arguments
parser = argparse.ArgumentParser(description='Exonum benchmarks util.')
parser.add_argument('--binaries-dir', dest='binaries_dir',
                    action="store", default="", help="path(empty or ends with '/') to the directory where tx_generator, timestamping and blockchain_utils are located")
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
                    action="store", default=100)
parser.add_argument('--tx-timeout', dest='tx_timeout', type=int,
                    action="store", default=100)
parser.add_argument('--propose-timeout', dest='propose_timeout', type=int,
                    action="store")
parser.add_argument('--start-port', dest='start_port', type=int,
                    action="store", help="is used when generating exonum config. same param as in 'timestamping generate'")
# todo do not hardcode 4 as node number and parametrize it
# parser.add_argument('--nodes-number', dest='nodes_number', type=int,
#                     action="store", default=4, help="number of nodes which will be started and processed. last node - is tx_generator")
args = parser.parse_args()

def print_args():
    # print arguments:
    print("binaries_dir: " + str(args.binaries_dir))
    print("exonum_dir: " + str(args.exonum_dir))
    print("benches_dir: " + str(args.benches_dir))
    print("tx_package_count: " + str(args.tx_package_count))
    print("tx_package_size_min: " + str(args.tx_package_size_min))
    print("tx_package_size_max: " + str(args.tx_package_size_max))
    print("tx_package_size_step: " + str(args.tx_package_size_step))
    print("tx_timeout: " + str(args.tx_timeout))
    print("propose_timeout: " + str(args.propose_timeout))
    print("start_port: " + str(args.start_port))
    # print("nodes_number: " + str(args.nodes_number))

def prepare_exonum_tmp_dir(exonum_tmp_dir_path):
    # clean tmp_dir
    shutil.rmtree(exonum_tmp_dir_path, ignore_errors=True)
    os.makedirs(exonum_tmp_dir_path)
    os.makedirs(exonum_tmp_dir_path + "/logs")
    os.makedirs(exonum_tmp_dir_path + "/db")
    os.makedirs(exonum_tmp_dir_path + "/benches")

def generate_exonum_config(exonum_tmp_dir_path):
    generate_args = [
        args.binaries_dir + "/timestamping",
        "generate",
        "4",
        "--output-dir", exonum_tmp_dir_path,
    ]
    if args.start_port is not None:
        generate_args.append("--start-port")
        generate_args.append(str(args.start_port))

    print("run generate_config with args: " + str(generate_args))
    generate_config_proc = Popen(generate_args,
                   stdout=DEVNULL, stderr=DEVNULL,
                   )
    generate_config_proc.communicate()

def update_exonum_config_with_propose_timeout(propose_timeout):
#     print config pathes
    for validator in range(0, 4):
        validator_config_file_path = args.exonum_dir + "/validators/{}.toml".format(validator)
        print(validator_config_file_path)
        for line in fileinput.input([validator_config_file_path], inplace=True):
            if line.strip().startswith("propose_timeout = "):
                line = "propose_timeout = {}\n".format(propose_timeout)
            sys.stdout.write(line)

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
        "--binaries-dir", "./target/release/examples/",
        # "--binaries-dir", "",
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

#       as an input param expected smth like this: "array('l', [0, 0, 0, 2])"
#       !! exactly 4 elements in array are expected
def get_numbers_of_unfound_txs_per_node(array_printed_as_string):
    result = []
    m = re.search(r"^.*[[\s](\d+).*[[\s](\d+).*[[\s](\d+).*[[\s](\d+)", array_printed_as_string)
    result.append(int(m.group(1)))
    result.append(int(m.group(2)))
    result.append(int(m.group(3)))
    result.append(int(m.group(4)))
    return result


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
    bench_output_file_path = benches_dir_path + "/" + count_of_unfound_txs_file_name
    with open(bench_output_file_path, "a") as myfile:
        print("current count_of_unfound_txs: " + str(get_count_of_unfound_txs(bench_dir_path)))
        numbers_of_unfound_txs_per_node = get_numbers_of_unfound_txs_per_node(str(get_count_of_unfound_txs(bench_dir_path)))
        print("current numbers_of_unfound_txs_per_node: " + str(numbers_of_unfound_txs_per_node))
        # myfile.write(bench_dir_name + " - " + get_count_of_unfound_txs(bench_dir_path))
        myfile.write(str(tx_count) + csv_delimiter)
        myfile.write(str(tx_package_size) + csv_delimiter)
        myfile.write(str(tx_timeout) + csv_delimiter)
        myfile.write(str(tx_count/tx_package_size) + csv_delimiter)
        for count_of_unfound_txs in numbers_of_unfound_txs_per_node:
            myfile.write(str(count_of_unfound_txs) + csv_delimiter)
        myfile.write("\n")
        myfile.close()


print_args()

# clean tmp_dir
prepare_exonum_tmp_dir(args.exonum_dir)

# generate exonum config
generate_exonum_config(args.exonum_dir)

if args.propose_timeout is not None:
    update_exonum_config_with_propose_timeout(args.propose_timeout)

# clean benches dir
shutil.rmtree(args.benches_dir, ignore_errors=True)
os.makedirs(args.benches_dir)

# prepare output csv file columns headers
bench_output_file_path = args.benches_dir + "/" + count_of_unfound_txs_file_name
with open(bench_output_file_path, "a") as myfile:
    myfile.write("tx_count" + csv_delimiter)
    myfile.write("tx_package_size" + csv_delimiter)
    myfile.write("tx_timeout" + csv_delimiter)
    myfile.write("number_of_expected_txs" + csv_delimiter)
    myfile.write("number_of_unfound_txs_in_node_0" + csv_delimiter)
    myfile.write("number_of_unfound_txs_in_node_1" + csv_delimiter)
    myfile.write("number_of_unfound_txs_in_node_2" + csv_delimiter)
    myfile.write("number_of_unfound_txs_in_node_3" + csv_delimiter)
    myfile.write("\n")
    myfile.close()

# loop through the range of flows
package_size_current = args.tx_package_size_min
while package_size_current <= args.tx_package_size_max:
    process_bench(args.exonum_dir, args.benches_dir, package_size_current * args.tx_package_count, package_size_current, args.tx_timeout)
    package_size_current += args.tx_package_size_step
    sleep(10)

# keep file with results in eorking directory
file_prefix_with_propose_timeout = ""
if args.propose_timeout is not None:
    file_prefix_with_propose_timeout = "propose_timeout-{}__".format(args.propose_timeout)
shutil.copy(bench_output_file_path, "./" + file_prefix_with_propose_timeout + count_of_unfound_txs_file_name)

