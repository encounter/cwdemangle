# cwdemangle [![Build Status]][actions] [![Latest Version]][crates.io] [![Api Rustdoc]][rustdoc] ![Rust Version]

[Build Status]: https://github.com/encounter/cwdemangle/actions/workflows/build.yaml/badge.svg
[actions]: https://github.com/encounter/cwdemangle/actions
[Latest Version]: https://img.shields.io/crates/v/cwdemangle.svg
[crates.io]: https://crates.io/crates/cwdemangle
[Api Rustdoc]: https://img.shields.io/badge/api-rustdoc-blue.svg
[rustdoc]: https://docs.rs/cwdemangle
[Rust Version]: https://img.shields.io/badge/rust-1.51+-blue.svg?maxAge=3600

A CodeWarrior C++ symbol demangler.

### Usage

CLI:

```shell
cwdemangle 'BuildLight__9CGuiLightCFv'
```

Library:

```rust
use cwdemangle::demangle;

if let Some(result) = demangle("BuildLight__9CGuiLightCFv") {
    println!("{}", result);
} else {
    eprintln!("Couldn't demangle symbol (not a C++ symbol?)");
}
```

### License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
