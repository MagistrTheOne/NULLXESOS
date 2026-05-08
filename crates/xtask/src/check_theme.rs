//! `xtask check-theme` — fail CI if any non-theme source uses a colour literal.
//!
//! Rule: `#?[0-9a-fA-F]{6}` matched against `Color::from_hex(0x...)` patterns
//! is allowed only inside `crates/theme/` and `theme.toml`. Everything else
//! must reference a named constant from `theme::color`.

use std::path::Path;

use anyhow::{anyhow, Context, Result};

const ALLOWED_PREFIXES: &[&str] = &[
    "crates/theme/",
    "tests/",
    "target/",
    ".git/",
    "node_modules/",
    "packaging/", // assets, generated theme outputs are committed under packaging/
    "iso/",
];

const FORBIDDEN_HEX: &str = "Color::from_hex(0x";

pub fn run() -> Result<()> {
    let root = repo_root()?;
    let mut violations: Vec<String> = Vec::new();

    walk(&root, &root, &mut violations)?;

    if violations.is_empty() {
        println!("xtask check-theme: clean");
        Ok(())
    } else {
        eprintln!("xtask check-theme: {} violation(s):", violations.len());
        for v in &violations { eprintln!("  {v}"); }
        Err(anyhow!("forbidden colour literal(s) outside theme crate"))
    }
}

fn repo_root() -> Result<std::path::PathBuf> {
    let cwd = std::env::current_dir().context("getcwd")?;
    let mut p = cwd.clone();
    loop {
        if p.join("Cargo.toml").exists() && p.join("crates").exists() {
            return Ok(p);
        }
        if !p.pop() {
            return Err(anyhow!("could not locate workspace root from {}", cwd.display()));
        }
    }
}

fn walk(root: &Path, dir: &Path, violations: &mut Vec<String>) -> Result<()> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("readdir {}", dir.display()))? {
        let entry = entry?;
        let path  = entry.path();
        let rel   = path.strip_prefix(root).unwrap_or(&path);
        let rel_str = rel.to_string_lossy().replace('\\', "/");

        if ALLOWED_PREFIXES.iter().any(|p| rel_str.starts_with(p)) {
            continue;
        }
        if path.is_dir() {
            walk(root, &path, violations)?;
        } else if matches!(path.extension().and_then(|s| s.to_str()), Some("rs")) {
            check_file(&rel_str, &path, violations)?;
        }
    }
    Ok(())
}

fn check_file(rel: &str, path: &Path, out: &mut Vec<String>) -> Result<()> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    for (i, line) in text.lines().enumerate() {
        if line.contains(FORBIDDEN_HEX) {
            out.push(format!("{rel}:{}: {}", i + 1, line.trim()));
        }
    }
    Ok(())
}
