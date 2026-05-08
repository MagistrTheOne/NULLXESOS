//! NULLXES `xtask` — repo automation entry point.
//!
//! Subcommands:
//!   - `check-theme`: lint forbidden hex literals outside the theme crate.
//!   - `iso`:         shim that delegates to `iso/scripts/build-iso.sh`.
//!   - `release`:     packages binaries + manifests for a tagged release.

#![deny(clippy::unwrap_used, clippy::expect_used)]

use anyhow::Result;
use clap::{Parser, Subcommand};

mod check_theme;

#[derive(Parser, Debug)]
#[command(name = "xtask", about = "NULLXES repo automation")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Fail if any non-theme source contains a 0xRRGGBB hex literal.
    CheckTheme,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::CheckTheme => check_theme::run(),
    }
}
