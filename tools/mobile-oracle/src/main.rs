use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};

fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let command = args
        .next()
        .context("usage: mobile-oracle verify <lock> <root> | diff <before-lock> <after-lock>")?;
    let first = PathBuf::from(args.next().context("missing first path")?);
    let second = PathBuf::from(args.next().context("missing second path")?);
    if args.next().is_some() {
        bail!("unexpected argument");
    }
    let output = if command == "verify" {
        serde_json::to_string_pretty(&mobile_oracle::verify(&first, &second)?)?
    } else if command == "diff" {
        serde_json::to_string_pretty(&mobile_oracle::compare_locks(&first, &second)?)?
    } else {
        bail!("unknown command; expected verify or diff");
    };
    writeln!(std::io::stdout().lock(), "{output}")?;
    Ok(())
}
