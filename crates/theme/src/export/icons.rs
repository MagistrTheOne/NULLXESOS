//! Icon theme generator — emits an `index.theme` that inherits Adwaita and
//! overrides nothing initially. Override icons get added in Phase 4 via the
//! `nullxes-icons` package.

use std::path::Path;

use anyhow::Context;

pub fn generate(_theme: &crate::Theme, out_dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(out_dir).context("icons out dir")?;
    let body = "[Icon Theme]\n\
                Name=NULLXES\n\
                Comment=NULLXES system icon theme\n\
                Inherits=Adwaita,hicolor\n\
                Directories=\n";
    std::fs::write(out_dir.join("index.theme"), body.as_bytes())?;
    Ok(())
}
