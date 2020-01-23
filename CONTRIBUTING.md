# Contributing to Exonum

Exonum welcomes contribution from everyone in the form of [suggestions],
[bug reports] or pull requests. This document gives some guidance if you are
thinking of helping us.

[Project overview] and [documentation] can help you to better understand current
state of the project.

You can always ask for help or guidance in our [gitter] channel. There is also
a separate [channel][gitter-ru] for our Russian-speaking users.

## Quick Start

Install Rust and mandatory dependencies according to our [installation guide],
then you can build the project and run the tests:

```shell
git clone https://github.com/exonum/exonum
cd exonum
cargo test --all
```

## Finding something to fix or improve

The [good first issue :heart:] label can be used to find the easy issues.

## Linters

Notice that the repository uses a set of linters for static analysis:

- [`clippy`]
- [`cargo-deadlinks`]
- [`rustfmt`]
- [`cspell`]
- [`markdownlint-cli`]

You can set up and run these tools locally (see [Travis script] for the details).

## Conventions

Generally, we follow common best practices established in the Rust community,
but we have several additional conventions:

- Create as minimal pull request as possible: they are easier to review and
  integrate.

  Additionally, we merge pull requests using the "squash and merge" strategy, so
  feel free to merge `master` branch in order to update instead of rebasing.

- Don't use `debug!` log level.

  It is convenient to use `debug!` when you develop some feature and are only
  interested in your logging output.

- Don't use `_` in public APIs, instead use full variable names and
  `#[allow(unused_variables)]`.

  Public APIs should be documented, but meaningful parameter names are also
  helpful for better understanding.

- Use [Rust 2018 edition].

  - Avoid `extern crate` unless it is required.
  - Use [anonymous lifetime].

- Prefer [nested imports] whenever possible.

- Don't try to minimize imports scope (for example don't put it inside a
  function), place them at the beginning of the file.

- Prefer importing a type name instead of using fully-qualified names.
  For example, avoid writing `std::path::Path` in code, instead import
  a type and use a shorter form:

  ```rust
  // Import type:
  use std::path::Path;

  // Use it:
  let path = Path::new("...");
  ```

- Modules and imports (`use`) should be in the following order:

  - `extern crate`s (if you really need them).
  - Reexporting (`pub use`).
  - Public modules (`pub mod`).
  - Imports (`use`):
    - Third-party libraries.
    - Standard library.
    - Internal.
  - Internal modules (`mod`).

[suggestions]: https://github.com/exonum/exonum/issues/new?template=feature.md
[bug reports]: https://github.com/exonum/exonum/issues/new?template=bug.md
[Project overview]: ARCHITECTURE.md
[documentation]: https://exonum.com/doc/
[gitter]: https://gitter.im/exonum/exonum
[gitter-ru]: https://gitter.im/exonum/ruExonum
[installation guide]: https://exonum.com/doc/version/latest/get-started/install/
[good first issue :heart:]: https://github.com/exonum/exonum/labels/good%20first%20issue%20%3Aheart%3A
[`clippy`]: https://github.com/rust-lang-nursery/rust-clippy
[`cargo-deadlinks`]: https://github.com/deadlinks/cargo-deadlinks
[`rustfmt`]: https://github.com/rust-lang-nursery/rustfmt
[`cspell`]: https://github.com/Jason3S/cspell
[`markdownlint-cli`]: https://github.com/igorshubovych/markdownlint-cli
[Travis script]: .travis.yml
[anonymous lifetime]: https://doc.rust-lang.org/edition-guide/rust-2018/ownership-and-lifetimes/the-anonymous-lifetime.html
[nested imports]: http://rust-lang.github.io/rfcs/2128-use-nested-groups.html
[Rust 2018 edition]: https://rust-lang-nursery.github.io/edition-guide/rust-2018/index.html
