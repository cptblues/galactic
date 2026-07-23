use bevy::prelude::*;

use crate::data::{SelectableId, SystemId};
use crate::state::ViewState;

#[derive(Clone, Debug, PartialEq)]
pub struct NavigationEntry {
    pub label: String,
    pub view: ViewState,
    pub selection: Option<SelectableId>,
    pub focus: Vec3,
    pub distance: f32,
    pub active_system: Option<SystemId>,
}

impl NavigationEntry {
    pub fn new(
        label: String,
        view: ViewState,
        selection: Option<SelectableId>,
        focus: Vec3,
        distance: f32,
        active_system: Option<SystemId>,
    ) -> Self {
        Self {
            label,
            view,
            selection,
            focus,
            distance,
            active_system,
        }
    }
}

#[derive(Resource, Clone, Debug)]
pub struct NavigationHistory {
    pub entries: Vec<NavigationEntry>,
    pub cursor: usize,
    pub limit: usize,
}

impl Default for NavigationHistory {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            cursor: 0,
            limit: 20,
        }
    }
}

impl NavigationHistory {
    pub fn push(&mut self, entry: NavigationEntry) {
        if self
            .entries
            .last()
            .is_some_and(|last| last.selection == entry.selection && last.view == entry.view)
        {
            return;
        }
        if self.cursor + 1 < self.entries.len() {
            self.entries.truncate(self.cursor + 1);
        }
        self.entries.push(entry);
        if self.entries.len() > self.limit {
            let overflow = self.entries.len() - self.limit;
            self.entries.drain(0..overflow);
        }
        self.cursor = self.entries.len().saturating_sub(1);
    }

    pub fn previous(&mut self) -> Option<NavigationEntry> {
        if self.entries.is_empty() || self.cursor == 0 {
            return None;
        }
        self.cursor -= 1;
        self.entries.get(self.cursor).cloned()
    }

    pub fn next(&mut self) -> Option<NavigationEntry> {
        if self.entries.is_empty() || self.cursor + 1 >= self.entries.len() {
            return None;
        }
        self.cursor += 1;
        self.entries.get(self.cursor).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::SystemId;

    fn entry(id: u32) -> NavigationEntry {
        NavigationEntry::new(
            format!("S{id}"),
            ViewState::Galaxy,
            Some(SelectableId::System(SystemId(id))),
            Vec3::ZERO,
            10.0,
            None,
        )
    }

    #[test]
    fn history_limits_entries_and_navigates() {
        let mut history = NavigationHistory {
            limit: 3,
            ..default()
        };
        history.push(entry(1));
        history.push(entry(2));
        history.push(entry(3));
        history.push(entry(4));
        assert_eq!(history.entries.len(), 3);
        assert_eq!(
            history.previous().unwrap().selection,
            Some(SelectableId::System(SystemId(3)))
        );
        assert_eq!(
            history.next().unwrap().selection,
            Some(SelectableId::System(SystemId(4)))
        );
    }

    #[test]
    fn history_truncates_forward_branch() {
        let mut history = NavigationHistory::default();
        history.push(entry(1));
        history.push(entry(2));
        history.push(entry(3));
        history.previous();
        history.push(entry(9));
        assert_eq!(
            history.entries.last().unwrap().selection,
            Some(SelectableId::System(SystemId(9)))
        );
        assert_eq!(history.entries.len(), 3);
    }
}
