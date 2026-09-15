use serde::{Deserialize, Serialize};
use crate::model::layout::LayoutTree;
use crate::model::session::SessionModel;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionLayoutTemplate {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub layout: LayoutTree,
}

impl SessionLayoutTemplate {
    pub fn new(name: impl Into<String>, description: Option<String>, layout: LayoutTree) -> Self {
        Self {
            name: name.into(),
            description,
            layout,
        }
    }

    pub fn from_session(name: &str, session: &SessionModel) -> Self {
        Self {
            name: name.to_string(),
            description: None,
            layout: session.layout.clone(),
        }
    }

    pub fn instantiate_session(&self, mut start_pane_id: u64, mut start_split_id: u64) -> SessionModel {
        let mut layout = self.layout.clone();
        layout.remap_ids(&mut start_pane_id, &mut start_split_id);
        let first_pane = layout
            .panes()
            .first()
            .copied()
            .unwrap_or(crate::model::PaneId(start_pane_id));
        SessionModel::from_layout(layout, first_pane, start_pane_id)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{PaneId, SplitOrientation};

    #[test]
    fn test_template_serialize_and_deserialize() {
        let mut tree = LayoutTree::new(PaneId(1));
        tree.split(PaneId(1), SplitOrientation::Horizontal, PaneId(2))
            .unwrap();
        tree.split(PaneId(2), SplitOrientation::Vertical, PaneId(3))
            .unwrap();

        let template = SessionLayoutTemplate::new(
            "2x2 Grid",
            Some("A 3-pane developer layout".into()),
            tree,
        );

        let json = template.to_json().unwrap();
        let loaded = SessionLayoutTemplate::from_json(&json).unwrap();
        assert_eq!(template, loaded);
        assert_eq!(loaded.name, "2x2 Grid");
        assert_eq!(loaded.description.as_deref(), Some("A 3-pane developer layout"));
        assert_eq!(loaded.layout.panes(), vec![PaneId(1), PaneId(2), PaneId(3)]);
    }

    #[test]
    fn test_template_instantiate_session_remaps_ids() {
        let mut session = SessionModel::new(PaneId(1));
        session.split_active(SplitOrientation::Horizontal).unwrap();
        session.split_active(SplitOrientation::Vertical).unwrap();

        let template = SessionLayoutTemplate::from_session("Dev Session", &session);

        let new_session = template.instantiate_session(100, 200);
        let panes = new_session.layout.panes();
        assert_eq!(panes, vec![PaneId(100), PaneId(101), PaneId(102)]);
        assert_eq!(new_session.active_pane, Some(PaneId(100)));

        // Splitting the new session continues from remapped next_pane_id and next_split_id
        let mut session_mut = new_session;
        let new_pane = session_mut.split_active(SplitOrientation::Horizontal).unwrap();
        assert_eq!(new_pane, PaneId(103));
    }
}
