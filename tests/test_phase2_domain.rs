#![allow(dead_code)]
#![allow(unused_imports)]

use tilix::model;


use model::layout::{LayoutNode, LayoutTree, PaneId, SplitId, SplitOrientation};
use model::profile::{CursorBlinkPreference, CursorShapePreference, Profile};
use model::session::SessionModel;
use model::template::SessionLayoutTemplate;
use model::theme::ColorScheme;

#[test]
fn test_integration_layout_ratio_persistence() {
    let mut tree = LayoutTree::new(PaneId(1));
    tree.split(PaneId(1), SplitOrientation::Horizontal, PaneId(2)).unwrap();
    tree.split(PaneId(2), SplitOrientation::Vertical, PaneId(3)).unwrap();

    // Verify initial split ratio is 0.5
    assert!(tree.set_split_ratio(SplitId(1), 0.65));
    assert!(tree.set_split_ratio(SplitId(2), 0.35));

    if let Some(LayoutNode::Split { id, ratio, second, .. }) = tree.root() {
        assert_eq!(*id, SplitId(1));
        assert!((ratio - 0.65).abs() < 1e-6);
        if let LayoutNode::Split { id: id2, ratio: ratio2, .. } = &**second {
            assert_eq!(*id2, SplitId(2));
            assert!((ratio2 - 0.35).abs() < 1e-6);
        } else {
            panic!("Expected second node to be split");
        }
    } else {
        panic!("Expected root to be split");
    }
}

#[test]
fn test_integration_template_export_and_import() {
    let mut session = SessionModel::new(PaneId(1));
    session.split_active(SplitOrientation::Horizontal).unwrap();
    session.split_active(SplitOrientation::Vertical).unwrap();

    let template = SessionLayoutTemplate::from_session("Production Dev", &session);
    let json = template.to_json().expect("Serialization should succeed");

    let restored_template = SessionLayoutTemplate::from_json(&json).expect("Deserialization should succeed");
    assert_eq!(restored_template.name, "Production Dev");

    // Instantiate with offset IDs to prevent any collisions
    let new_session = restored_template.instantiate_session(50, 100);
    assert_eq!(new_session.layout.panes(), vec![PaneId(50), PaneId(51), PaneId(52)]);
    assert_eq!(new_session.active_pane, Some(PaneId(50)));
}

#[test]
fn test_integration_theme_and_profile() {
    let mut profile = Profile::default();
    assert_eq!(profile.name, "Default");
    assert_eq!(profile.color_scheme.name, "Tilix Dark");

    // Switch to Solarized Dark
    profile.color_scheme = ColorScheme::solarized_dark();
    profile.cursor_shape = CursorShapePreference::Underline;
    profile.cursor_blink = CursorBlinkPreference::On;

    let json = serde_json::to_string_pretty(&profile).unwrap();
    let deserialized: Profile = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.name, "Default");
    assert_eq!(deserialized.color_scheme.name, "Solarized Dark");
    assert_eq!(deserialized.cursor_shape, CursorShapePreference::Underline);
    assert_eq!(deserialized.cursor_blink, CursorBlinkPreference::On);
}

#[test]
fn test_integration_session_sync_broadcast_logic() {
    let mut session = SessionModel::new(PaneId(1));
    let p2 = session.split_active(SplitOrientation::Horizontal).unwrap();
    let p3 = session.split_active(SplitOrientation::Vertical).unwrap();

    assert!(!session.sync_input_enabled);
    session.toggle_sync_input();
    assert!(session.sync_input_enabled);

    // Default: all panes are sync-enabled
    assert!(session.is_pane_sync_enabled(PaneId(1)));
    assert!(session.is_pane_sync_enabled(p2));
    assert!(session.is_pane_sync_enabled(p3));

    // Disable sync on p3
    session.set_pane_sync_enabled(p3, false);
    assert!(!session.is_pane_sync_enabled(p3));

    // Determine broadcast targets if typing in p1
    let sender = PaneId(1);
    let targets: Vec<PaneId> = session
        .layout
        .panes()
        .into_iter()
        .filter(|&id| id != sender && session.is_pane_sync_enabled(id))
        .collect();

    assert_eq!(targets, vec![p2]);
}

#[test]
fn test_integration_session_close_active_and_fallback() {
    let mut session = SessionModel::new(PaneId(1));
    let p2 = session.split_active(SplitOrientation::Horizontal).unwrap();
    assert_eq!(session.active_pane, Some(p2));

    // Closing active pane p2 should fallback active to p1
    assert!(session.close_pane(p2).is_ok());
    assert_eq!(session.active_pane, Some(PaneId(1)));
    assert_eq!(session.layout.panes(), vec![PaneId(1)]);
}
