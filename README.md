# cwdemangle [![Build Status]][actions]

[Build Status]: https://github.com/encounter/cwdemangle/workflows/build/badge.svg
[actions]: https://github.com/encounter/cwdemangle/actions

A CodeWarrior C++ symbol demangler.

### Usage

CLI:

```shell
cwdemangle 'BuildLight__9CGuiLightCFv'
```

Library:

```rust
use cwdemangle::demangle;

let symbol = "BuildLight__9CGuiLightCFv";
if let Some(result) = demangle(symbol) {
    println!("{}", result);
    Ok(())
} else {
    Err("Couldn't demangle symbol (not a C++ symbol?)")
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
