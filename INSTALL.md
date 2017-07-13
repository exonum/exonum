# Build instructions
First of all, you need to install system dependencies.

# Installing system dependencies
The crate uses these system libraries:
* [leveldb](https://github.com/google/leveldb)
* [libsodium](https://download.libsodium.org/doc/)
* [openssl](https://www.openssl.org)

Below you can find instructions on how to obtain them for the different operating systems:

## macOS 
If you use `homebrew` you can simply install needed libraries with the command:
```shell
brew install libsodium leveldb openssl
```

## Linux
For deb based systems like Debian or Ubuntu you need the following packages:
```shell
apt install build-essential libsodium-dev libleveldb-dev libssl-dev pkg-config
```
Other linux users may find the packages with similar names in their package managers.

## Windows
Building and workability is not guaranteed yet.

# Installing Rust
The project uses a stable Rust version that can be installed by using the [rustup](https://www.rustup.rs) utility.

```shell
curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain stable
```

Nightly (`2017-07-05`) version is used for [clippy](https://github.com/Manishearth/rust-clippy). You can install it with the following command:
```shell
rustup toolchain install nightly-2017-07-05
```
And run Clippy checks this way:
```shell
cargo +nightly-2017-07-05 clippy
```

# Compiling the project 
You can verify that you installed everything correctly by compiling the `exonum` crate and run tests suite with the command:
```shell
cargo test --manifest-path exonum/Cargo.toml
```
You may want to launch the extended tests suite which is named `sandbox`.
```shell
cargo test --manifest-path sandbox/Cargo.toml
```
After all this you can learn how to create your own blockchain solution.
