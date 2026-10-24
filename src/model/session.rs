use crate::model::layout::{Direction, DockPosition, LayoutError, LayoutTree, PaneId, SplitOrientation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SyncGroupId(pub u32);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionModel {
    pub layout: LayoutTree,
    pub active_pane: Option<PaneId>,
    pub sync_groups: HashMap<PaneId, SyncGroupId>,
    #[serde(default)]
    pub sync_input_enabled: bool,
    #[serde(default)]
    pub pane_sync_overrides: HashMap<PaneId, bool>,
    #[serde(default)]
    pub focus_history: Vec<PaneId>,
    #[serde(default)]
    pub maximized_pane: Option<PaneId>,
    next_pane_id: u64,
}

impl SessionModel {
    pub fn new(initial_pane: PaneId) -> Self {
        Self {
            layout: LayoutTree::new(initial_pane),
            active_pane: Some(initial_pane),
            sync_groups: HashMap::new(),
            sync_input_enabled: false,
            pane_sync_overrides: HashMap::new(),
            focus_history: vec![initial_pane],
            maximized_pane: None,
            next_pane_id: initial_pane.0 + 1,
        }
    }

    pub fn from_layout(layout: LayoutTree, active_pane: PaneId, next_pane_id: u64) -> Self {
        Self {
            layout,
            active_pane: Some(active_pane),
            sync_groups: HashMap::new(),
            sync_input_enabled: false,
            pane_sync_overrides: HashMap::new(),
            focus_history: vec![active_pane],
            maximized_pane: None,
            next_pane_id,
        }
    }

    pub fn next_pane_id(&mut self) -> PaneId {
        let id = PaneId(self.next_pane_id);
        self.next_pane_id += 1;
        id
    }

    pub fn is_pane_sync_enabled(&self, pane_id: PaneId) -> bool {
        *self.pane_sync_overrides.get(&pane_id).unwrap_or(&true)
    }

    pub fn set_pane_sync_enabled(&mut self, pane_id: PaneId, enabled: bool) {
        self.pane_sync_overrides.insert(pane_id, enabled);
    }

    pub fn toggle_sync_input(&mut self) -> bool {
        self.sync_input_enabled = !self.sync_input_enabled;
        self.sync_input_enabled
    }

    pub fn record_focus(&mut self, id: PaneId) {
        self.focus_history.retain(|&p| p != id);
        self.focus_history.push(id);
    }

    pub fn set_active_pane(&mut self, id: PaneId) {
        if self.layout.contains(id) {
            self.active_pane = Some(id);
            self.record_focus(id);
        }
    }

    pub fn is_maximized(&self) -> bool {
        self.maximized_pane.is_some()
    }

    pub fn maximized_pane(&self) -> Option<PaneId> {
        self.maximized_pane
    }

    pub fn toggle_maximize(&mut self, target: Option<PaneId>) -> bool {
        if self.layout.panes().len() <= 1 {
            return false;
        }

        if let Some(current) = self.maximized_pane {
            match target {
                None => {
                    self.maximized_pane = None;
                    false
                }
                Some(id) if id == current => {
                    self.maximized_pane = None;
                    false
                }
                Some(id) => {
                    if self.layout.contains(id) {
                        self.maximized_pane = Some(id);
                        self.set_active_pane(id);
                        true
                    } else {
                        false
                    }
                }
            }
        } else {
            let candidate = target.or(self.active_pane);
            if let Some(id) = candidate {
                if self.layout.contains(id) {
                    self.maximized_pane = Some(id);
                    self.set_active_pane(id);
                    return true;
                }
            }
            false
        }
    }

    pub fn unmaximize(&mut self) -> bool {
        if self.maximized_pane.is_some() {
            self.maximized_pane = None;
            true
        } else {
            false
        }
    }

    pub fn split_pane(
        &mut self,
        target: PaneId,
        orientation: SplitOrientation,
    ) -> Result<PaneId, LayoutError> {
        let new_id = self.next_pane_id();
        self.layout.split(target, orientation, new_id)?;
        self.maximized_pane = None;
        self.set_active_pane(new_id);
        Ok(new_id)
    }

    pub fn split_active(
        &mut self,
        orientation: SplitOrientation,
    ) -> Result<PaneId, LayoutError> {
        let active = self.active_pane.ok_or(LayoutError::InvalidSplit)?;
        self.split_pane(active, orientation)
    }

    pub fn remove_pane(&mut self, id: PaneId) -> Result<Option<PaneId>, LayoutError> {
        let fallback_focus = self.layout.close(id)?;
        self.sync_groups.remove(&id);
        self.pane_sync_overrides.remove(&id);
        self.focus_history.retain(|&p| p != id);

        if self.maximized_pane == Some(id) || self.layout.panes().len() <= 1 {
            self.maximized_pane = None;
        }

        if self.active_pane == Some(id) {
            let next_focus = self
                .focus_history
                .last()
                .copied()
                .filter(|&p| self.layout.contains(p))
                .or(fallback_focus);
            self.active_pane = next_focus;
            Ok(next_focus)
        } else {
            // When closing or removing another pane, the active pane remains unchanged
            Ok(self.active_pane)
        }
    }

    pub fn close_pane(&mut self, id: PaneId) -> Result<Option<PaneId>, LayoutError> {
        self.remove_pane(id)
    }

    pub fn dock_pane(
        &mut self,
        source: PaneId,
        target: PaneId,
        position: DockPosition,
    ) -> Result<(), LayoutError> {
        self.layout.dock_pane(source, target, position)?;
        self.maximized_pane = None;
        self.set_active_pane(source);
        Ok(())
    }

    pub fn adopt_pane(
        &mut self,
        new_pane: PaneId,
        target: PaneId,
        position: DockPosition,
    ) -> Result<(), LayoutError> {
        self.layout.insert_pane_dock(new_pane, target, position)?;
        if new_pane.0 >= self.next_pane_id {
            self.next_pane_id = new_pane.0 + 1;
        }
        self.maximized_pane = None;
        self.set_active_pane(new_pane);
        Ok(())
    }

    pub fn focus_adjacent(&mut self, direction: Direction) -> Option<PaneId> {
        if self.is_maximized() {
            return None;
        }
        let active = self.active_pane?;
        if let Some(adjacent) = self.layout.find_adjacent(active, direction) {
            self.set_active_pane(adjacent);
            Some(adjacent)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_model_split_and_close() {
        let mut session = SessionModel::new(PaneId(1));
        assert_eq!(session.active_pane, Some(PaneId(1)));

        let new_id = session.split_active(SplitOrientation::Horizontal).unwrap();
        assert_eq!(new_id, PaneId(2));
        assert_eq!(session.active_pane, Some(PaneId(2)));

        let next = session.close_pane(PaneId(2)).unwrap();
        assert_eq!(next, Some(PaneId(1)));
        assert_eq!(session.active_pane, Some(PaneId(1)));

        let empty = session.close_pane(PaneId(1)).unwrap();
        assert_eq!(empty, None);
        assert_eq!(session.active_pane, None);
        assert!(session.layout.panes().is_empty());
        assert_eq!(session.layout.root(), None);
    }

    #[test]
    fn test_session_sync_model() {
        let mut session = SessionModel::new(PaneId(1));
        assert!(!session.sync_input_enabled);

        // Toggle sync
        assert!(session.toggle_sync_input());
        assert!(session.sync_input_enabled);

        let p2 = session.split_active(SplitOrientation::Horizontal).unwrap();
        assert!(session.is_pane_sync_enabled(PaneId(1)));
        assert!(session.is_pane_sync_enabled(p2));

        // Disable sync override on p2
        session.set_pane_sync_enabled(p2, false);
        assert!(!session.is_pane_sync_enabled(p2));
        assert!(session.is_pane_sync_enabled(PaneId(1)));

        // Closing p2 cleans up overrides
        session.close_pane(p2).unwrap();
        assert!(!session.pane_sync_overrides.contains_key(&p2));
    }

    #[test]
    fn test_session_focus_history_on_split_and_switch() {
        let mut session = SessionModel::new(PaneId(1));
        assert_eq!(session.active_pane, Some(PaneId(1)));
        assert_eq!(session.focus_history, vec![PaneId(1)]);

        // Split active (1 -> 2)
        let p2 = session.split_active(SplitOrientation::Horizontal).unwrap();
        assert_eq!(session.active_pane, Some(p2));
        assert_eq!(session.focus_history, vec![PaneId(1), PaneId(2)]);

        // Split active (2 -> 3)
        let p3 = session.split_active(SplitOrientation::Vertical).unwrap();
        assert_eq!(session.active_pane, Some(p3));
        assert_eq!(session.focus_history, vec![PaneId(1), PaneId(2), PaneId(3)]);

        // Switch focus back to 1
        session.set_active_pane(PaneId(1));
        assert_eq!(session.active_pane, Some(PaneId(1)));
        // 1 moved to the end of MRU history
        assert_eq!(session.focus_history, vec![PaneId(2), PaneId(3), PaneId(1)]);
    }

    #[test]
    fn test_session_close_other_pane_preserves_active_pane() {
        let mut session = SessionModel::new(PaneId(1));
        let p2 = session.split_active(SplitOrientation::Horizontal).unwrap();
        let p3 = session.split_active(SplitOrientation::Vertical).unwrap();

        // Switch focus to 1
        session.set_active_pane(PaneId(1));
        assert_eq!(session.active_pane, Some(PaneId(1)));
        assert_eq!(session.focus_history, vec![p2, p3, PaneId(1)]);

        // Close pane 2 (not active)
        let res = session.close_pane(p2).unwrap();
        assert_eq!(res, Some(PaneId(1)));
        // Active pane is STILL pane 1!
        assert_eq!(session.active_pane, Some(PaneId(1)));
        // p2 is removed from history
        assert_eq!(session.focus_history, vec![p3, PaneId(1)]);

        // Close pane 3 (not active)
        let res2 = session.close_pane(p3).unwrap();
        assert_eq!(res2, Some(PaneId(1)));
        assert_eq!(session.active_pane, Some(PaneId(1)));
        assert_eq!(session.focus_history, vec![PaneId(1)]);
    }

    #[test]
    fn test_session_close_active_pane_restores_previous_pane() {
        let mut session = SessionModel::new(PaneId(1));
        let p2 = session.split_active(SplitOrientation::Horizontal).unwrap();
        let p3 = session.split_active(SplitOrientation::Vertical).unwrap();

        // History is [1, 2, 3], active is 3
        assert_eq!(session.active_pane, Some(p3));

        // Closing current pane 3 should automatically move focus to previous pane 2
        let next = session.close_pane(p3).unwrap();
        assert_eq!(next, Some(p2));
        assert_eq!(session.active_pane, Some(p2));
        assert_eq!(session.focus_history, vec![PaneId(1), p2]);

        // Closing current pane 2 should move focus to previous pane 1
        let next2 = session.close_pane(p2).unwrap();
        assert_eq!(next2, Some(PaneId(1)));
        assert_eq!(session.active_pane, Some(PaneId(1)));
        assert_eq!(session.focus_history, vec![PaneId(1)]);

        // Closing last pane 1 returns None
        let next3 = session.close_pane(PaneId(1)).unwrap();
        assert_eq!(next3, None);
        assert_eq!(session.active_pane, None);
        assert!(session.focus_history.is_empty());
    }

    #[test]
    fn test_session_focus_history_mru_ordering() {
        let mut session = SessionModel::new(PaneId(1));
        let p2 = session.split_active(SplitOrientation::Horizontal).unwrap();
        let p3 = session.split_active(SplitOrientation::Vertical).unwrap();

        // Visit sequence: 1 -> 2 -> 3 -> switch to 1 -> switch to 2
        session.set_active_pane(PaneId(1));
        session.set_active_pane(p2);
        // MRU order is now: [3, 1, 2]
        assert_eq!(session.focus_history, vec![p3, PaneId(1), p2]);
        assert_eq!(session.active_pane, Some(p2));

        // Closing current pane 2 should move focus to previous pane 1 (the one active before 2)
        let next = session.close_pane(p2).unwrap();
        assert_eq!(next, Some(PaneId(1)));
        assert_eq!(session.active_pane, Some(PaneId(1)));

        // Closing current pane 1 should move focus to pane 3
        let next2 = session.close_pane(PaneId(1)).unwrap();
        assert_eq!(next2, Some(p3));
        assert_eq!(session.active_pane, Some(p3));
    }

    #[test]
    fn test_session_focus_adjacent_records_history() {
        let mut session = SessionModel::new(PaneId(1));
        let p2 = session.split_active(SplitOrientation::Horizontal).unwrap();
        assert_eq!(session.active_pane, Some(p2));

        // Navigate left from p2 to pane 1
        let adjacent = session.focus_adjacent(Direction::Left);
        assert_eq!(adjacent, Some(PaneId(1)));
        assert_eq!(session.active_pane, Some(PaneId(1)));
        assert_eq!(session.focus_history, vec![p2, PaneId(1)]);
    }

    #[test]
    fn test_session_serialization_backwards_compatibility() {
        // Old json without focus_history field
        let json = r#"{
            "layout": {"Leaf": 1},
            "active_pane": 1,
            "sync_groups": {},
            "sync_input_enabled": false,
            "pane_sync_overrides": {},
            "next_pane_id": 2
        }"#;

        let session: SessionModel = serde_json::from_str(json).unwrap();
        assert_eq!(session.active_pane, Some(PaneId(1)));
        assert!(session.focus_history.is_empty());
    }

    #[test]
    fn test_session_model_remove_pane() {
        let mut session = SessionModel::new(PaneId(1));
        let p2 = session.split_active(SplitOrientation::Horizontal).unwrap();
        assert_eq!(session.active_pane, Some(p2));
        assert_eq!(session.layout.panes(), vec![PaneId(1), p2]);

        // Remove p2
        let fallback = session.remove_pane(p2).unwrap();
        assert_eq!(fallback, Some(PaneId(1)));
        assert_eq!(session.active_pane, Some(PaneId(1)));
        assert_eq!(session.layout.panes(), vec![PaneId(1)]);
        assert!(!session.layout.contains(p2));
        assert!(!session.focus_history.contains(&p2));

        // Remove last pane
        let last = session.remove_pane(PaneId(1)).unwrap();
        assert_eq!(last, None);
        assert_eq!(session.active_pane, None);
        assert!(session.layout.panes().is_empty());
    }

    #[test]
    fn test_session_model_dock_pane() {
        let mut session = SessionModel::new(PaneId(1));
        let p2 = session.split_active(SplitOrientation::Horizontal).unwrap();
        assert_eq!(session.layout.panes(), vec![PaneId(1), p2]);

        // Dock p1 to bottom of p2
        session.dock_pane(PaneId(1), p2, DockPosition::Bottom).unwrap();
        assert_eq!(session.layout.panes(), vec![p2, PaneId(1)]);
        assert_eq!(session.active_pane, Some(PaneId(1)));
    }

    #[test]
    fn test_session_model_adopt_pane() {
        let mut session = SessionModel::new(PaneId(1));
        session.adopt_pane(PaneId(99), PaneId(1), DockPosition::Left).unwrap();
        assert_eq!(session.layout.panes(), vec![PaneId(99), PaneId(1)]);
        assert_eq!(session.active_pane, Some(PaneId(99)));
        assert!(session.next_pane_id().0 > 99);
    }

    #[test]
    fn test_session_model_maximize_single_pane_noop() {
        let mut session = SessionModel::new(PaneId(1));
        assert!(!session.is_maximized());
        assert_eq!(session.maximized_pane(), None);

        // Maximize single pane should be a no-op
        assert!(!session.toggle_maximize(None));
        assert!(!session.is_maximized());
        assert_eq!(session.maximized_pane(), None);

        assert!(!session.toggle_maximize(Some(PaneId(1))));
        assert!(!session.is_maximized());
        assert_eq!(session.maximized_pane(), None);

        assert!(!session.unmaximize());
    }

    #[test]
    fn test_session_model_maximize_toggle_and_unmaximize() {
        let mut session = SessionModel::new(PaneId(1));
        let p2 = session.split_active(SplitOrientation::Horizontal).unwrap();
        assert_eq!(session.active_pane, Some(p2));

        // Maximize active pane (p2)
        assert!(session.toggle_maximize(None));
        assert!(session.is_maximized());
        assert_eq!(session.maximized_pane(), Some(p2));

        // Toggle again unmaximizes
        assert!(!session.toggle_maximize(None));
        assert!(!session.is_maximized());
        assert_eq!(session.maximized_pane(), None);

        // Maximize specific pane (p1)
        assert!(session.toggle_maximize(Some(PaneId(1))));
        assert!(session.is_maximized());
        assert_eq!(session.maximized_pane(), Some(PaneId(1)));
        assert_eq!(session.active_pane, Some(PaneId(1)));

        // Unmaximize explicitly
        assert!(session.unmaximize());
        assert!(!session.is_maximized());
        assert_eq!(session.maximized_pane(), None);
        assert!(!session.unmaximize());
    }

    #[test]
    fn test_session_model_maximize_switch_target() {
        let mut session = SessionModel::new(PaneId(1));
        let p2 = session.split_active(SplitOrientation::Horizontal).unwrap();
        let p3 = session.split_active(SplitOrientation::Vertical).unwrap();
        assert_eq!(session.layout.panes().len(), 3);

        // Maximize pane 1
        assert!(session.toggle_maximize(Some(PaneId(1))));
        assert_eq!(session.maximized_pane(), Some(PaneId(1)));
        assert_eq!(session.active_pane, Some(PaneId(1)));

        // Switch maximize directly to pane 2
        assert!(session.toggle_maximize(Some(p2)));
        assert_eq!(session.maximized_pane(), Some(p2));
        assert_eq!(session.active_pane, Some(p2));

        // Switch maximize directly to pane 3
        assert!(session.toggle_maximize(Some(p3)));
        assert_eq!(session.maximized_pane(), Some(p3));
        assert_eq!(session.active_pane, Some(p3));

        // Switch to non-existent pane should fail and not switch
        assert!(!session.toggle_maximize(Some(PaneId(999))));
        assert_eq!(session.maximized_pane(), Some(p3));

        // Toggle on same pane unmaximizes
        assert!(!session.toggle_maximize(Some(p3)));
        assert!(!session.is_maximized());
        assert_eq!(session.maximized_pane(), None);
    }

    #[test]
    fn test_session_model_maximize_split_auto_unmaximize() {
        let mut session = SessionModel::new(PaneId(1));
        let _p2 = session.split_active(SplitOrientation::Horizontal).unwrap();

        assert!(session.toggle_maximize(Some(PaneId(1))));
        assert!(session.is_maximized());

        // Split active or target auto-unmaximizes
        let p3 = session.split_pane(PaneId(1), SplitOrientation::Vertical).unwrap();
        assert!(!session.is_maximized());
        assert_eq!(session.maximized_pane(), None);
        assert_eq!(session.active_pane, Some(p3));
    }

    #[test]
    fn test_session_model_maximize_close_auto_unmaximize() {
        let mut session = SessionModel::new(PaneId(1));
        let p2 = session.split_active(SplitOrientation::Horizontal).unwrap();
        let p3 = session.split_active(SplitOrientation::Vertical).unwrap();

        // Maximize p3 and close it
        assert!(session.toggle_maximize(Some(p3)));
        assert!(session.is_maximized());
        session.close_pane(p3).unwrap();
        assert!(!session.is_maximized());
        assert_eq!(session.maximized_pane(), None);

        // Maximize p1 in 2-pane session
        assert!(session.toggle_maximize(Some(PaneId(1))));
        assert!(session.is_maximized());

        // Close peer p2 -> leaves only 1 pane -> auto unmaximize
        session.close_pane(p2).unwrap();
        assert!(!session.is_maximized());
        assert_eq!(session.maximized_pane(), None);
    }

    #[test]
    fn test_session_model_maximize_serde_roundtrip() {
        let mut session = SessionModel::new(PaneId(1));
        let p2 = session.split_active(SplitOrientation::Horizontal).unwrap();
        session.toggle_maximize(Some(p2));
        assert_eq!(session.maximized_pane(), Some(p2));

        let json = serde_json::to_string(&session).unwrap();
        let deserialized: SessionModel = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.maximized_pane, Some(p2));
        assert!(deserialized.is_maximized());

        // Backward compatibility without maximized_pane
        let legacy_json = r#"{
            "layout": {"Leaf": 1},
            "active_pane": 1,
            "sync_groups": {},
            "sync_input_enabled": false,
            "pane_sync_overrides": {},
            "focus_history": [1],
            "next_pane_id": 2
        }"#;
        let legacy: SessionModel = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(legacy.maximized_pane, None);
        assert!(!legacy.is_maximized());
    }

    #[test]
    fn test_session_model_maximize_focus_adjacent_guard() {
        let mut session = SessionModel::new(PaneId(1));
        let p2 = session.split_active(SplitOrientation::Horizontal).unwrap();
        // Currently p2 is active, p1 is left
        assert_eq!(session.focus_adjacent(Direction::Left), Some(PaneId(1)));
        assert_eq!(session.active_pane, Some(PaneId(1)));

        // Maximize pane 1
        assert!(session.toggle_maximize(Some(PaneId(1))));
        assert!(session.is_maximized());

        // Attempting to focus adjacent right must be blocked (return None) and not change active_pane
        assert_eq!(session.focus_adjacent(Direction::Right), None);
        assert_eq!(session.active_pane, Some(PaneId(1)));
        assert_eq!(session.maximized_pane(), Some(PaneId(1)));

        // Once unmaximized, focus_adjacent works again
        session.unmaximize();
        assert_eq!(session.focus_adjacent(Direction::Right), Some(p2));
        assert_eq!(session.active_pane, Some(p2));
    }
}
