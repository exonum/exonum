# idea of the file is to provide helper function which gets name of (binary)file and path to folder (root of binaries)
# search in root of binaries and it's subfolders and return path to found binary file
# throw an exception if file is not found or is found more than once

import glob

def call_find_file_in_binaries_folder(binaries_root_path, binary_name):

    # check input argument
    if not binaries_root_path.endswith("/"):
        binaries_root_path += "/"

    # search for asked file in provided folder recursively
    found_files = []
    # for filename in glob.iglob('./target/**/timestamping' , recursive=True):
    for filename in glob.iglob(binaries_root_path + "**/" + binary_name, recursive=True):
        found_files.append(filename)

    if len(found_files) == 0:
        raise Exception('binary {} is not found in folder {}'.format(binary_name, binaries_root_path))

    if len(found_files) > 1:
        raise Exception('more than one file {} is found in folder {}: {}'.format(binary_name, binaries_root_path, str(found_files)))

    return str(found_files[0])
