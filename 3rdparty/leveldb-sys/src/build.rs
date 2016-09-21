use std::env;

fn main() {

    if let Ok(lib_dir) = env::var("LEVELDB_LIB_DIR") {

    	println!("cargo:rustc-flags=-L native={}", lib_dir);

        let mode = match env::var_os("LEVELDB_STATIC") {
            Some(_) => "static",
            None => "dylib"
        };
        println!("cargo:rustc-flags=-l {0}=leveldb", mode);

    } else {
        println!("cargo:rustc-flags=-l leveldb");
    }
}