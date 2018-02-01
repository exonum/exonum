A plugin to insert appropriate `flame::start_guard(_)` calls (for use with
[flame](https://github.com/TyOverby/flame))

[![Build Status](https://travis-ci.org/llogiq/flamer.svg)](https://travis-ci.org/llogiq/flamer)
[![Current Version](https://img.shields.io/crates/v/flamer.svg)](https://crates.io/crates/flamer)
[![Docs](https://docs.rs/flamer/badge.svg)](https://docs.rs/flamer)

**This needs a nightly rustc!** Because flamer is a compiler plugin, it uses
unstable APIs, which are not available on stable or beta. It may be possible to
extend flamer to allow use with syntex, but this hasn't been tried yet.

Usage:

In your Cargo.toml add `flame` and `flamer` to your dependencies:

```toml
[dependencies]
flame = "^0.1.9"
flamer = "^0.1.4"
```

Then in your crate root, add the following:

```rust
#![feature(plugin, custom_attribute)]
#![plugin(flamer)]

extern crate flame;
```

You may also opt for an *optional dependency*. In that case your Cargo.toml should have:

```toml
[dependencies]
flame = { version = "^0.1.9", optional = true }
flamer = { version = "^0.1.4", optional = true }

[features]
default = []
flame_it = ["flame", "flamer"]
```

And your crate root should contain:

```rust
#![cfg_attr(feature="flame_it", feature(plugin, custom_attribute))]
#![cfg_attr(feature="flame_it", plugin(flamer))]

#[cfg(feature="flame_it")]
extern crate flame;

// as well as the following instead of `#[flame]`
#[cfg_attr(feature="flame_it", flame)];
```

You should then be able to annotate every item (or even the whole crate) with
`#[flame]` annotations. You can also use `#[noflame]` annotations to disable
instrumentations for subitems of `#[flame]`d items. Note that this only
instruments the annotated methods, it does not print out the results.

Refer to flame's documentation to see how output works.

License: Apache 2.0
