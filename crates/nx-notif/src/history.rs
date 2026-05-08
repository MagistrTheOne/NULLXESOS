//! Notification history persistence.
//!
//! File: `$XDG_STATE_HOME/nullxes/notifications.json`
//! Format: append-mode JSON-lines (one record per line). On startup we load the
//! tail of the file; if size > `HISTORY_CAP`, we rotate by writing the last
//! `HISTORY_CAP / 2` bytes to a `.0` rotation.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::surface::{Notif, Urgency};

const HISTORY_CAP: u64 = 1_048_576; // 1 MiB

#[derive(Debug, Serialize, Deserialize)]
struct Record {
    id:       u32,
    app:      String,
    summary:  String,
    body:     String,
    urgency:  u8,
    received: u64,
}

pub struct History {
    path:   PathBuf,
    file:   File,
    cached: Vec<Record>,
}

impl History {
    pub fn append(&mut self, n: &Notif) -> std::io::Result<()> {
        let rec = Record {
            id:       n.id,
            app:      n.app_name.clone(),
            summary:  n.summary.clone(),
            body:     n.body.clone(),
            urgency:  match n.urgency { Urgency::Low => 0, Urgency::Normal => 1, Urgency::Critical => 2 },
            received: now_unix_ms(),
        };
        let mut line = serde_json::to_string(&rec).unwrap_or_default();
        line.push('\n');
        self.file.write_all(line.as_bytes())?;
        self.file.flush()?;

        let pos = self.file.seek(SeekFrom::End(0))?;
        if pos > HISTORY_CAP {
            self.rotate()?;
        }

        self.cached.push(rec);
        if self.cached.len() > 256 {
            self.cached.drain(0..self.cached.len() - 256);
        }
        Ok(())
    }

    fn rotate(&mut self) -> std::io::Result<()> {
        let rotation = self.path.with_extension("json.0");
        std::fs::rename(&self.path, &rotation)?;
        self.file = OpenOptions::new()
            .create(true).read(true).append(true)
            .open(&self.path)?;
        // Replay the last 256 entries from rotation into the new file as a
        // bootstrap, so on next startup we still have recent history.
        if let Ok(reader) = File::open(&rotation).map(BufReader::new) {
            for line in reader.lines().filter_map(Result::ok).rev().take(256).collect::<Vec<_>>().into_iter().rev() {
                writeln!(self.file, "{line}")?;
            }
        }
        Ok(())
    }
}

pub fn open_or_create() -> anyhow::Result<History> {
    let dir = state_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("notifications.json");
    let file = OpenOptions::new()
        .create(true).read(true).append(true)
        .open(&path)?;

    // Load last 256 records.
    let mut cached: Vec<Record> = Vec::new();
    if let Ok(reader) = File::open(&path).map(BufReader::new) {
        for line in reader.lines().filter_map(Result::ok) {
            if let Ok(rec) = serde_json::from_str::<Record>(&line) {
                cached.push(rec);
                if cached.len() > 256 { cached.remove(0); }
            }
        }
    }

    Ok(History { path, file, cached })
}

fn state_dir() -> PathBuf {
    if let Ok(p) = std::env::var("XDG_STATE_HOME") {
        return PathBuf::from(p).join("nullxes");
    }
    if let Some(home) = dirs_next::home_dir() {
        return home.join(".local/state/nullxes");
    }
    PathBuf::from("/tmp/nullxes")
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
