//! `theme-export` CLI — generates branding assets for a NULLXES install.
//!
//! Invoked from PKGBUILDs:
//!     theme-export gtk         --out /usr/share/themes/NULLXES/
//!     theme-export qt6         --out /usr/share/qt6ct/colors/
//!     theme-export plymouth    --out /usr/share/plymouth/themes/nullxes/
//!     theme-export systemd-boot --out /boot/loader/entries/
//!     theme-export greetd      --out /usr/share/nullxes-greet/
//!     theme-export icons       --out /usr/share/icons/NULLXES/
//!
//! The `--theme` flag points to a `theme.toml`; defaults to `/etc/nullxes/theme.toml`,
//! falling back to the embedded default tokens.

#![deny(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use theme::Theme;

#[derive(Parser, Debug)]
#[command(name = "theme-export", version, about = "Generate NULLXES branding assets from theme tokens.")]
struct Cli {
    /// Path to theme.toml (uses defaults if missing).
    #[arg(long, global = true, default_value = "/etc/nullxes/theme.toml")]
    theme: PathBuf,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    Gtk { #[arg(long)] out: PathBuf },
    Qt6 { #[arg(long)] out: PathBuf },
    Plymouth { #[arg(long)] out: PathBuf },
    SystemdBoot { #[arg(long)] out: PathBuf },
    Greetd { #[arg(long)] out: PathBuf },
    Icons { #[arg(long)] out: PathBuf },
    /// Emit every artifact under <out>/{themes,qt6,plymouth,systemd-boot,greetd,icons}.
    All { #[arg(long)] out: PathBuf },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let theme = Theme::load_or_default(&cli.theme);
    match cli.cmd {
        Cmd::Gtk { out }         => theme::export::gtk::generate(&theme, &out)?,
        Cmd::Qt6 { out }         => theme::export::qt::generate(&theme, &out)?,
        Cmd::Plymouth { out }    => theme::export::plymouth::generate(&theme, &out)?,
        Cmd::SystemdBoot { out } => theme::export::systemd_boot::generate(&theme, &out)?,
        Cmd::Greetd { out }      => theme::export::greetd::generate(&theme, &out)?,
        Cmd::Icons { out }       => theme::export::icons::generate(&theme, &out)?,
        Cmd::All { out } => {
            theme::export::gtk::generate(&theme,         &out.join("themes/NULLXES"))?;
            theme::export::qt::generate(&theme,          &out.join("qt6"))?;
            theme::export::plymouth::generate(&theme,    &out.join("plymouth/nullxes"))?;
            theme::export::systemd_boot::generate(&theme,&out.join("systemd-boot"))?;
            theme::export::greetd::generate(&theme,      &out.join("greetd"))?;
            theme::export::icons::generate(&theme,       &out.join("icons/NULLXES"))?;
        }
    }
    Ok(())
}
