//! Asset generation from theme tokens.
//!
//! Determinism: every generator is a pure function of the input tokens. We
//! honour `SOURCE_DATE_EPOCH` for any embedded timestamps. The same `theme.toml`
//! always produces byte-identical outputs — verified by the
//! `tests/export_determinism.rs` integration test.

pub mod gtk;
pub mod qt;
pub mod plymouth;
pub mod systemd_boot;
pub mod greetd;
pub mod icons;
