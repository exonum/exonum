# Build instructions
First of all, you need to install system dependencies.

# Installing system dependencies
The crate uses these system libraries:
* [leveldb](https://github.com/google/leveldb)
* [libsodium](https://download.libsodium.org/doc/)
* [openssl](https://www.openssl.org)

Below you can find instructions on how to obtain them for the different operating systems:

## macOS 
If you use `homebrew` you can simple install needed libraries with the command:
```shell
brew install libsodium leveldb openssl
```

## Linux
For deb based systems like Debian or Ubuntu you need to the following packages:
```shell
apt install build-essential libsodium-dev libleveldb-dev libssl-dev
```
Other linux users may find the packages with similar names in their package managers.

## Windows
Workability is not yet guaranteed.

# Installing Rust
The project uses a nightly rust version that can be installed by using the [rustup](https://www.rustup.rs) utility.

```shell
curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain nightly
```

The latest working version is dated by `2017-01-08`. You can set it with the command:
```shell
rustup override set nightly-2017-01-08
```

# Compiling the project 
You can verify that you installed everything correctly by compiling the `exonum-core` crate with the command:
```shell
cargo test --manifest-path exonum/Cargo.toml
```
You may want to launch extended tests suite which named `sandbox`.
```shell
cargo test --manifest-path sandbox/Cargo.toml
```
After all this you can learn how the create your own blockchain solution.
