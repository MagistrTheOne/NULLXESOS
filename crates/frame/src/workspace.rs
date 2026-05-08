//! Workspace data model.
//!
//! Two cooperating types:
//!   - [`WorkspaceManager`] holds the *index* state (which workspace is active,
//!     how many exist, focused window per workspace).
//!   - [`Workspaces`] holds one `Space<Window>` per workspace and is the single
//!     source of truth for window membership and Z-order.
//!
//! `switch_to(idx)` just flips the active index. `move_focused_to(target)`
//! unmaps the focused window from the active space and re-maps it on the target.

use smithay::desktop::{Space, Window};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(pub u64);

pub struct Workspaces {
    spaces: Vec<Space<Window>>,
}

impl Workspaces {
    pub fn new(count: u8) -> Self {
        let count = count.clamp(1, 9) as usize;
        Self {
            spaces: (0..count).map(|_| Space::default()).collect(),
        }
    }

    pub fn count(&self) -> usize { self.spaces.len() }

    pub fn active(&self, idx: usize) -> &Space<Window> {
        // Out-of-bounds indices clamp to 0 so we never panic. WorkspaceManager
        // already guarantees this in normal operation; the clamp guards against
        // races on hot-reload of workspace.count.
        self.spaces.get(idx).unwrap_or(&self.spaces[0])
    }

    pub fn active_mut(&mut self, idx: usize) -> &mut Space<Window> {
        let idx = idx.min(self.spaces.len().saturating_sub(1));
        &mut self.spaces[idx]
    }

    pub fn get(&self, idx: usize) -> Option<&Space<Window>> { self.spaces.get(idx) }
    pub fn get_mut(&mut self, idx: usize) -> Option<&mut Space<Window>> { self.spaces.get_mut(idx) }

    pub fn occupied(&self) -> Vec<usize> {
        self.spaces
            .iter()
            .enumerate()
            .filter(|(_, s)| s.elements().next().is_some())
            .map(|(i, _)| i)
            .collect()
    }
}

pub struct WorkspaceManager {
    count:   usize,
    active:  usize,
    focused: Vec<Option<WindowId>>,
}

impl WorkspaceManager {
    pub fn new(count: u8) -> Self {
        let count = count.clamp(1, 9) as usize;
        Self {
            count,
            active: 0,
            focused: vec![None; count],
        }
    }

    pub fn count(&self) -> usize { self.count }
    pub fn active_index(&self) -> usize { self.active }

    pub fn switch_to(&mut self, idx: usize) -> bool {
        if idx >= self.count || idx == self.active {
            return false;
        }
        self.active = idx;
        tracing::debug!(workspace = idx, "switched workspace");
        true
    }

    pub fn switch_next(&mut self, wrap: bool) -> bool {
        let next = self.active + 1;
        if next < self.count { self.switch_to(next) }
        else if wrap         { self.switch_to(0) }
        else                 { false }
    }

    pub fn switch_prev(&mut self, wrap: bool) -> bool {
        if self.active > 0 { self.switch_to(self.active - 1) }
        else if wrap       { self.switch_to(self.count - 1) }
        else               { false }
    }

    pub fn focused(&self) -> Option<WindowId> {
        self.focused.get(self.active).copied().flatten()
    }

    pub fn set_focus(&mut self, id: Option<WindowId>) {
        if let Some(slot) = self.focused.get_mut(self.active) {
            *slot = id;
        }
    }

    /// Returns `Some((src_workspace_idx, target_workspace_idx))` if a move was
    /// scheduled — caller is expected to perform the actual `Space` re-map.
    pub fn plan_move_focused(&mut self, target: usize) -> Option<(usize, usize, WindowId)> {
        if target >= self.count || target == self.active {
            return None;
        }
        let id = self.focused.get(self.active).copied().flatten()?;
        let src = self.active;
        // Move bookkeeping atomically; caller does the Space mutation.
        self.focused[src] = None;
        self.focused[target] = Some(id);
        Some((src, target, id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switch_and_wrap() {
        let mut m = WorkspaceManager::new(3);
        assert_eq!(m.active_index(), 0);
        assert!(m.switch_to(2));
        assert_eq!(m.active_index(), 2);
        assert!(!m.switch_next(false));
        assert!(m.switch_next(true));
        assert_eq!(m.active_index(), 0);
    }

    #[test]
    fn move_plan_returns_indices() {
        let mut m = WorkspaceManager::new(2);
        m.set_focus(Some(WindowId(7)));
        let plan = m.plan_move_focused(1);
        assert_eq!(plan, Some((0, 1, WindowId(7))));
        m.switch_to(1);
        assert_eq!(m.focused(), Some(WindowId(7)));
    }

    #[test]
    fn out_of_range_move_is_noop() {
        let mut m = WorkspaceManager::new(2);
        m.set_focus(Some(WindowId(1)));
        assert!(m.plan_move_focused(5).is_none());
        assert_eq!(m.focused(), Some(WindowId(1)));
    }
}
