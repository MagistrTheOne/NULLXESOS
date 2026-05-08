//! PTY + alacritty_terminal integration.
//!
//! `TerminalSession` wires up:
//!   - `alacritty_terminal::tty::Pty` driven by `EventLoop`.
//!   - A shared `Term<TerminalEvents>` protected by `parking_lot::Mutex`.
//!   - A `mpsc::Sender<()>` "dirty" channel that the Wayland thread polls.

use std::sync::{mpsc, Arc};

use alacritty_terminal::{
    event::{Event as AlacEvent, EventListener, Notify, OnResize, WindowSize},
    event_loop::{EventLoop, Msg as AlacMsg, Notifier},
    grid::Dimensions,
    sync::FairMutex,
    term::{cell::Cell, Config as TermConfig, Term},
    tty::{self, Options as PtyOptions},
};

#[derive(Clone)]
pub struct TerminalEvents {
    pub dirty_tx: mpsc::Sender<()>,
}

impl EventListener for TerminalEvents {
    fn send_event(&self, event: AlacEvent) {
        match event {
            AlacEvent::Wakeup
            | AlacEvent::Title(_)
            | AlacEvent::ResetTitle
            | AlacEvent::ColorRequest(..)
            | AlacEvent::Bell
            | AlacEvent::ChildExit(_)
            | AlacEvent::ClipboardLoad(..)
            | AlacEvent::ClipboardStore(..) => {
                let _ = self.dirty_tx.send(());
            }
            _ => {}
        }
    }
}

pub struct TerminalSession {
    pub term:     Arc<FairMutex<Term<TerminalEvents>>>,
    pub notifier: Notifier,
    pub _pty_thread: std::thread::JoinHandle<(EventLoop<tty::Pty, TerminalEvents>, alacritty_terminal::event_loop::State)>,
}

impl TerminalSession {
    pub fn new(
        cols:     u16,
        rows:     u16,
        cell_w:   u32,
        cell_h:   u32,
        dirty_tx: mpsc::Sender<()>,
    ) -> anyhow::Result<Self> {
        let pty_opts = PtyOptions {
            shell:     None, // fall back to passwd shell / SHELL env / /bin/sh
            working_directory: None,
            hold:      false,
            env:       std::collections::HashMap::new(),
        };
        let window_size = WindowSize {
            num_cols: cols,
            num_lines: rows,
            cell_width:  cell_w as u16,
            cell_height: cell_h as u16,
        };
        let pty = tty::new(&pty_opts, window_size, 0)?;

        let cfg = TermConfig::default();
        let listener = TerminalEvents { dirty_tx };
        let dim = AlacDims { cols: cols as usize, rows: rows as usize };
        let term = Term::new(cfg, &dim, listener.clone());
        let term = Arc::new(FairMutex::new(term));

        let event_loop = EventLoop::new(
            term.clone(),
            listener,
            pty,
            false, // hold_after_exit
            false, // ref_test
        )?;
        let notifier = Notifier(event_loop.channel());
        let pty_thread = event_loop.spawn();

        Ok(Self { term, notifier, _pty_thread: pty_thread })
    }

    pub fn write(&self, bytes: &[u8]) {
        self.notifier.notify(bytes.to_vec());
    }

    pub fn resize(&self, cols: u16, rows: u16, cell_w: u32, cell_h: u32) {
        let size = WindowSize {
            num_cols: cols,
            num_lines: rows,
            cell_width:  cell_w as u16,
            cell_height: cell_h as u16,
        };
        self.notifier.on_resize(size);
        let mut t = self.term.lock();
        t.resize(AlacDims { cols: cols as usize, rows: rows as usize });
    }
}

/// alacritty_terminal expects a `Dimensions` view of the grid; we provide the
/// minimal one here (no padding handling — kept to the renderer).
#[derive(Clone, Copy)]
pub struct AlacDims {
    pub cols: usize,
    pub rows: usize,
}

impl Dimensions for AlacDims {
    fn columns(&self) -> usize { self.cols }
    fn screen_lines(&self) -> usize { self.rows }
    fn total_lines(&self) -> usize { self.rows }
}

/// Helper for the renderer to access the visible cell grid.
pub fn snapshot_grid(term: &Term<TerminalEvents>) -> Vec<Vec<Cell>> {
    let cols = term.columns();
    let rows = term.screen_lines();
    let mut out = Vec::with_capacity(rows);
    for line in 0..rows {
        let mut row = Vec::with_capacity(cols);
        for col in 0..cols {
            let p = alacritty_terminal::index::Point::new(
                alacritty_terminal::index::Line(line as i32),
                alacritty_terminal::index::Column(col),
            );
            row.push(term.grid()[p].clone());
        }
        out.push(row);
    }
    out
}
