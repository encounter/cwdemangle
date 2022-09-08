use argh::FromArgs;
use cwdemangle::demangle;

use crate::argh_cargo::from_env;

mod argh_cargo;

#[derive(FromArgs)]
/// A CodeWarrior C++ symbol demangler.
struct Args {
    /// the symbol to demangle
    #[argh(positional)]
    symbol: String,
}

fn main() -> Result<(), &'static str> {
    let args: Args = from_env();
    return if let Some(symbol) = demangle(args.symbol.as_str()) {
        println!("{}", symbol);
        Ok(())
    } else {
        Err("Failed to demangle symbol")
    };
}
