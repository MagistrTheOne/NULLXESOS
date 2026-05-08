//! Determinism guard: same theme.toml → byte-identical export output.

#![cfg(feature = "cli")]

use std::path::PathBuf;

use theme::{export, Theme};

fn workspace_tmp(name: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("nullxes-theme-export-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("mkdir tmp");
    p
}

#[test]
fn gtk_byte_identical_twice() {
    let theme = Theme::default();
    let a = workspace_tmp("gtk-a");
    let b = workspace_tmp("gtk-b");
    export::gtk::generate(&theme, &a).expect("a");
    export::gtk::generate(&theme, &b).expect("b");
    let css_a = std::fs::read(a.join("gtk-3.0/gtk.css")).expect("read a");
    let css_b = std::fs::read(b.join("gtk-3.0/gtk.css")).expect("read b");
    assert_eq!(css_a, css_b);
}

#[test]
fn qt_byte_identical_twice() {
    let theme = Theme::default();
    let a = workspace_tmp("qt-a");
    let b = workspace_tmp("qt-b");
    export::qt::generate(&theme, &a).expect("a");
    export::qt::generate(&theme, &b).expect("b");
    let conf_a = std::fs::read(a.join("nullxes.conf")).expect("read a");
    let conf_b = std::fs::read(b.join("nullxes.conf")).expect("read b");
    assert_eq!(conf_a, conf_b);
}
