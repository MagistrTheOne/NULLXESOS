//! Fuzzy search over the discovered application list.

use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};

use crate::apps::AppEntry;

pub struct Searcher {
    matcher: SkimMatcherV2,
    apps:    Vec<AppEntry>,
}

impl Searcher {
    pub fn new(apps: Vec<AppEntry>) -> Self {
        Self {
            matcher: SkimMatcherV2::default().smart_case(),
            apps,
        }
    }

    /// Returns up to `limit` apps matching `query`, sorted by score desc.
    /// Empty query → first `limit` entries (alphabetical).
    pub fn query(&self, query: &str, limit: usize) -> Vec<&AppEntry> {
        if query.is_empty() {
            return self.apps.iter().take(limit).collect();
        }
        let mut scored: Vec<(i64, &AppEntry)> = self
            .apps
            .iter()
            .filter_map(|app| {
                self.matcher
                    .fuzzy_match(&app.name, query)
                    .map(|score| (score, app))
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.into_iter().take(limit).map(|(_, a)| a).collect()
    }

    pub fn count(&self) -> usize { self.apps.len() }
}
