# Fuzz testing suit for Exonum Core

Fuzz testing is a software testing technique used to find security and stability
 issues by providing pseudo-random data as input to the software.

## Install

At the moment, Exonum uses [`libFuzzer`] via [`cargo-fuzz`]. To install, type

```bash
cargo install cargo-fuzz --force
```

in your terminal.

[`libFuzzer`]: http://llvm.org/docs/LibFuzzer.html
[`cargo-fuzz`]: https://github.com/rust-fuzz/cargo-fuzz

## Fuzz targets

Fuzz target is an executable which will receive some bytes from the fuzzer
and perform some specific actions to test API against this input.

You can look at [example target][example] which tests [`RawMessage::from_vec`]
function.

[example]: fuzz_targets/raw_message.rs
[`RawMessage::from_vec`]: https://docs.rs/exonum/0.5.1/exonum/messages/struct.RawMessage.html#method.from_vec

To add new target, run

```bash
cargo fuzz add target_name
```

`Cargo-fuzz` will generate some basic executable inside `fuzz_targets`
directory.

You will need to add your code inside `fuzz_target!` macro call:

```rust
fuzz_target!(|data: &[u8]| {
    perform_testing(data);
});
```

To run fuzz target, type

```bash
cargo fuzz run target_name
```

You can also pass additional command-line arguments to the `libFuzzer`, using
the following syntax:

```bash
cargo fuzz run target_name -- -help=1
```

## Interpreting results

After some time, your fuzz target will probably abort the execution due to
unexpected panic in your code. You will be able to find input data that led to
a crash inside `artifacts/target_name` directory.

To test your code against suspicious input, run

```bash
cargo fuzz run target_name /path/to/input
```

When you find an issue, please send us the information about this by
[Project Issue Tracker](https://github.com/exonum/exonum/issues/new) and
provide all the necessary data to reproduce the crash.

## It's not enough

For further reading, please see
[Rust Fuzzing Authority Book](https://rust-fuzz.github.io/book/).
