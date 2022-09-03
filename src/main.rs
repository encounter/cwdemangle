use argh::FromArgs;

use cwdemangle::demangle;

#[derive(FromArgs)]
/// CodeWarrior C++ demangler
struct Args {
    /// the symbol to demangle
    #[argh(positional)]
    symbol: String,
}

fn main() -> Result<(), &'static str> {
    let args: Args = argh::from_env();
    return if let Some(symbol) = demangle(args.symbol.as_str()) {
        println!("{}", symbol);
        Ok(())
    } else {
        Err("Failed to demangle symbol")
    };
}
