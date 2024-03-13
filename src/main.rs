use argh::FromArgs;
use cwdemangle::{demangle, DemangleOptions};

use crate::argh_cargo::from_env;

mod argh_cargo;

#[derive(FromArgs)]
/// A CodeWarrior C++ symbol demangler.
struct Args {
    /// the symbol to demangle
    #[argh(positional)]
    symbol: String,
    /// disable replacing `(void)` with `()`
    #[argh(switch)]
    keep_void: bool,
    /// enable Metrowerks extensions
    #[argh(switch)]
    mw_extensions: bool,
}

fn main() -> Result<(), &'static str> {
    let args: Args = from_env();
    return if let Some(symbol) = demangle(args.symbol.as_str(), &DemangleOptions {
        omit_empty_parameters: !args.keep_void,
        mw_extensions: args.mw_extensions,
    }) {
        println!("{symbol}");
        Ok(())
    } else {
        Err("Failed to demangle symbol")
    };
}
