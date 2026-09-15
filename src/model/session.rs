use crate::model::layout::{Direction, LayoutError, LayoutTree, PaneId, SplitOrientation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SyncGroupId(pub u32);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionModel {
    pub layout: LayoutTree,
    pub active_pane: Option<PaneId>,
    pub sync_groups: HashMap<PaneId, SyncGroupId>,
    next_pane_id: u64,
}

impl SessionModel {
    pub fn new(initial_pane: PaneId) -> Self {
        Self {
            layout: LayoutTree::new(initial_pane),
            active_pane: Some(initial_pane),
            sync_groups: HashMap::new(),
            next_pane_id: initial_pane.0 + 1,
        }
    }

    pub fn next_pane_id(&mut self) -> PaneId {
        let id = PaneId(self.next_pane_id);
        self.next_pane_id += 1;
        id
    }

    pub fn split_pane(
        &mut self,
        target: PaneId,
        orientation: SplitOrientation,
    ) -> Result<PaneId, LayoutError> {
        let new_id = self.next_pane_id();
        self.layout.split(target, orientation, new_id)?;
        self.active_pane = Some(new_id);
        Ok(new_id)
    }

    pub fn split_active(
        &mut self,
        orientation: SplitOrientation,
    ) -> Result<PaneId, LayoutError> {
        let active = self.active_pane.ok_or(LayoutError::InvalidSplit)?;
        self.split_pane(active, orientation)
    }

    pub fn close_pane(&mut self, id: PaneId) -> Result<Option<PaneId>, LayoutError> {
        let next_focus = self.layout.close(id)?;
        self.sync_groups.remove(&id);
        if self.active_pane == Some(id) {
            self.active_pane = next_focus;
        }
        Ok(next_focus)
    }

    pub fn focus_adjacent(&mut self, direction: Direction) -> Option<PaneId> {
        let active = self.active_pane?;
        if let Some(adjacent) = self.layout.find_adjacent(active, direction) {
            self.active_pane = Some(adjacent);
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
}
