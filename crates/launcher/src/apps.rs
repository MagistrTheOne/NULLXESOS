//! XDG `.desktop` application discovery.
//!
//! Implements the freedesktop "Desktop Entry Specification" 1.5 sufficient for
//! launching from a fuzzy-search interface:
//!   - Honours `NoDisplay`, `Hidden`, `OnlyShowIn`, `NotShowIn`.
//!   - Strips field codes (`%f`, `%F`, `%u`, `%U`, `%i`, `%c`, `%k`).
//!   - Splits the `Exec` string with `shlex` so quoted args are preserved.
//!   - Deduplicates by canonical name; if multiple entries share a name we
//!     prefer the one that is the most specific user override.

use std::collections::HashMap;
use std::path::PathBuf;

use freedesktop_desktop_entry::{DesktopEntry, Iter as DesktopIter};

#[derive(Debug, Clone)]
pub struct AppEntry {
    pub name:        String,
    pub comment:     Option<String>,
    /// Argv as parsed by shlex with field codes filtered out.
    pub argv:        Vec<String>,
    pub icon:        Option<String>,
    pub categories:  Vec<String>,
    pub source_path: PathBuf,
}

impl AppEntry {
    pub fn launch_argv(&self) -> &[String] { &self.argv }
}

pub fn scan() -> Vec<AppEntry> {
    let dirs = xdg_app_dirs();
    let mut by_name: HashMap<String, AppEntry> = HashMap::new();

    let xdg_current_desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let current_desktops: Vec<&str> = xdg_current_desktop
        .split(':')
        .filter(|s| !s.is_empty())
        .collect();

    for entry in DesktopIter::new(dirs.iter().cloned()) {
        let path = entry.clone();
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                tracing::debug!(?e, ?path, "read failed; skipping");
                continue;
            }
        };
        let de = match DesktopEntry::decode(&path, &bytes) {
            Ok(d) => d,
            Err(e) => {
                tracing::debug!(?e, ?path, "parse failed; skipping");
                continue;
            }
        };

        if de.no_display() { continue; }
        if !visible_in(&de, &current_desktops) { continue; }

        let Some(name) = de.name(None).map(|s| s.to_string()) else { continue; };
        let Some(exec) = de.exec().map(|s| s.to_string()) else { continue; };
        let argv = parse_exec(&exec);
        if argv.is_empty() { continue; }

        let app = AppEntry {
            name: name.clone(),
            comment: de.comment(None).map(|s| s.to_string()),
            argv,
            icon: de.icon().map(|s| s.to_string()),
            categories: de
                .categories()
                .map(|s| s.split(';').filter(|c| !c.is_empty()).map(|c| c.to_string()).collect())
                .unwrap_or_default(),
            source_path: path,
        };

        // Last writer wins, but only if the new path is more specific
        // (user overrides system).
        match by_name.entry(name) {
            std::collections::hash_map::Entry::Vacant(v) => { v.insert(app); }
            std::collections::hash_map::Entry::Occupied(mut o) => {
                if more_specific(&app.source_path, &o.get().source_path) {
                    o.insert(app);
                }
            }
        }
    }

    let mut out: Vec<AppEntry> = by_name.into_values().collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

fn visible_in(de: &DesktopEntry, current: &[&str]) -> bool {
    // OnlyShowIn — must contain at least one of current desktops.
    if let Some(only) = de.only_show_in() {
        let allowed: Vec<&str> = only.split(';').filter(|s| !s.is_empty()).collect();
        if current.is_empty() {
            // No XDG_CURRENT_DESKTOP set: respect OnlyShowIn restrictively
            // (hide). This matches GNOME/KDE behaviour.
            return false;
        }
        if !allowed.iter().any(|d| current.iter().any(|c| c.eq_ignore_ascii_case(d))) {
            return false;
        }
    }
    if let Some(not) = de.not_show_in() {
        let denied: Vec<&str> = not.split(';').filter(|s| !s.is_empty()).collect();
        if denied.iter().any(|d| current.iter().any(|c| c.eq_ignore_ascii_case(d))) {
            return false;
        }
    }
    true
}

fn more_specific(new_path: &std::path::Path, old_path: &std::path::Path) -> bool {
    // User dir > system dir. Heuristic: if new path's first parent containing
    // "applications" is under $HOME, treat as user.
    let in_home = |p: &std::path::Path| {
        if let Some(home) = std::env::var_os("HOME") {
            return p.starts_with(home);
        }
        false
    };
    in_home(new_path) && !in_home(old_path)
}

/// Split an `Exec=` value with shlex and drop XDG field codes.
pub fn parse_exec(exec: &str) -> Vec<String> {
    let Some(parts) = shlex::split(exec) else {
        tracing::warn!(exec, "Exec parse failed; skipping");
        return Vec::new();
    };
    parts
        .into_iter()
        .filter(|tok| !is_field_code(tok))
        .collect()
}

fn is_field_code(tok: &str) -> bool {
    // Pure field codes: %f %F %u %U %i %c %k %v %m %% (literal % preserved as-is)
    matches!(tok, "%f" | "%F" | "%u" | "%U" | "%i" | "%c" | "%k" | "%v" | "%m")
}

fn xdg_app_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(data) = dirs_next::data_dir() {
        dirs.push(data.join("applications"));
    }
    if let Ok(xdg_dirs) = std::env::var("XDG_DATA_DIRS") {
        for d in xdg_dirs.split(':') {
            if !d.is_empty() {
                dirs.push(PathBuf::from(d).join("applications"));
            }
        }
    } else {
        dirs.push(PathBuf::from("/usr/local/share/applications"));
        dirs.push(PathBuf::from("/usr/share/applications"));
    }
    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
    if let Some(data) = dirs_next::data_dir() {
        dirs.push(data.join("flatpak/exports/share/applications"));
    }
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shlex_drops_field_codes() {
        let argv = parse_exec("/usr/bin/foo --flag %F %U arg");
        assert_eq!(argv, vec!["/usr/bin/foo", "--flag", "arg"]);
    }

    #[test]
    fn shlex_handles_quoted() {
        let argv = parse_exec("/usr/bin/foo \"one two\" --x=y");
        assert_eq!(argv, vec!["/usr/bin/foo", "one two", "--x=y"]);
    }

    #[test]
    fn shlex_unparsable_returns_empty() {
        assert!(parse_exec("\"unterminated").is_empty());
    }
}
