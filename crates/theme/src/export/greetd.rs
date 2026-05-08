//! Greeter assets: small NULLXES wordmark for the login overlay background.

use std::path::Path;

use anyhow::Context;

pub fn generate(_theme: &crate::Theme, out_dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(out_dir).context("greet out dir")?;
    // Stage 1 ships only an env file the NX-GREET reads at startup; assets
    // are derived at runtime from the theme tokens.
    let env = "# NULLXES NX-GREET configuration (read by /usr/bin/nullxes-greet)\n\
                # Optional startup wallpaper for greeter:\n\
                # NULLXES_GREET_BACKGROUND=/usr/share/nullxes/backgrounds/default.png\n";
    std::fs::write(out_dir.join("greet.env"), env.as_bytes())?;
    Ok(())
}
