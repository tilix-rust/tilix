#![allow(dead_code)]
#![allow(unused_imports)]

use std::path::PathBuf;

#[path = "../src/model/mod.rs"]
mod model;

#[path = "../src/pty/mod.rs"]
mod pty;

#[path = "../src/ui/mod.rs"]
mod ui;

#[path = "../src/app.rs"]
mod app;

use app::{parse_cli_args, CliAction};
use model::config::AppConfig;
use model::layout::{LayoutNode, LayoutTree, PaneId, SplitId, SplitOrientation};
use model::profile::{CursorBlinkPreference, CursorShapePreference, Profile};
use model::theme::ColorScheme;
use pty::shell::parse_osc7_uri;

#[test]
fn test_integration_osc7_uri_parsing_all_variants() {
    // Standard file URI
    assert_eq!(
        parse_osc7_uri("file:///home/developer/workspace"),
        Some(PathBuf::from("/home/developer/workspace"))
    );

    // URI with localhost hostname
    assert_eq!(
        parse_osc7_uri("file://localhost/home/developer/workspace"),
        Some(PathBuf::from("/home/developer/workspace"))
    );

    // Percent-encoded spaces and special characters
    assert_eq!(
        parse_osc7_uri("file:///home/developer/My%20Projects/Sub%20Folder"),
        Some(PathBuf::from("/home/developer/My Projects/Sub Folder"))
    );

    // Root directory
    assert_eq!(parse_osc7_uri("file:///"), Some(PathBuf::from("/")));
    assert_eq!(parse_osc7_uri("file://localhost/"), Some(PathBuf::from("/")));

    // Invalid schemes
    assert_eq!(parse_osc7_uri("http://localhost/path"), None);
    assert_eq!(parse_osc7_uri("ftp://localhost/path"), None);
    assert_eq!(parse_osc7_uri("/bare/path/not/uri"), None);
    assert_eq!(parse_osc7_uri("file://"), None);
    assert_eq!(parse_osc7_uri(""), None);
}

#[test]
fn test_integration_layout_swap_panes_deep_tree() {
    let mut tree = LayoutTree::new(PaneId(1));
    tree.split(PaneId(1), SplitOrientation::Horizontal, PaneId(2))
        .unwrap();
    tree.split(PaneId(2), SplitOrientation::Vertical, PaneId(3))
        .unwrap();
    tree.split(PaneId(1), SplitOrientation::Vertical, PaneId(4))
        .unwrap();

    // Panes: [1, 4, 2, 3]
    assert_eq!(
        tree.panes(),
        vec![PaneId(1), PaneId(4), PaneId(2), PaneId(3)]
    );

    // Set custom split ratios
    assert!(tree.set_split_ratio(SplitId(1), 0.70));
    assert!(tree.set_split_ratio(SplitId(2), 0.30));

    // Swap nested leaf 4 with leaf 3
    tree.swap_panes(PaneId(4), PaneId(3)).unwrap();
    assert_eq!(
        tree.panes(),
        vec![PaneId(1), PaneId(3), PaneId(2), PaneId(4)]
    );

    // Verify split ratios and tree structure are preserved
    if let Some(LayoutNode::Split { id, ratio, .. }) = tree.root() {
        assert_eq!(*id, SplitId(1));
        assert!((ratio - 0.70).abs() < 1e-6);
    } else {
        panic!("Root must be a split");
    }

    // Swap back
    tree.swap_panes(PaneId(3), PaneId(4)).unwrap();
    assert_eq!(
        tree.panes(),
        vec![PaneId(1), PaneId(4), PaneId(2), PaneId(3)]
    );

    // Same pane swap is no-op
    tree.swap_panes(PaneId(1), PaneId(1)).unwrap();
    assert_eq!(
        tree.panes(),
        vec![PaneId(1), PaneId(4), PaneId(2), PaneId(3)]
    );

    // Non-existent pane returns error
    assert!(tree.swap_panes(PaneId(1), PaneId(999)).is_err());
    assert!(tree.swap_panes(PaneId(999), PaneId(1)).is_err());
}

#[test]
fn test_integration_cli_args_parsing() {
    assert_eq!(parse_cli_args(Vec::<&str>::new()), CliAction::NewWindow);
    assert_eq!(parse_cli_args(["tilix"]), CliAction::NewWindow);
    assert_eq!(
        parse_cli_args(["tilix", "--quake"]),
        CliAction::QuakeShow
    );
    assert_eq!(
        parse_cli_args(["tilix", "--quake-toggle"]),
        CliAction::QuakeToggle
    );
    assert_eq!(
        parse_cli_args(["tilix", "--preferences"]),
        CliAction::Preferences
    );
    assert_eq!(parse_cli_args(["tilix", "-p"]), CliAction::Preferences);
    assert_eq!(parse_cli_args(["tilix", "--help"]), CliAction::Help);
    assert_eq!(parse_cli_args(["tilix", "-h"]), CliAction::Help);
    assert_eq!(parse_cli_args(["tilix", "--version"]), CliAction::Version);
    assert_eq!(parse_cli_args(["tilix", "-v"]), CliAction::Version);
}

#[test]
fn test_integration_app_config_persistence_and_mutation() {
    let mut config = AppConfig::default();
    assert_eq!(config.quake_height_percent, 40);
    assert!(!config.quake_hide_on_unfocus);
    assert!(config.notifications_enabled);

    // Mutate configuration
    config.quake_height_percent = 60;
    config.quake_hide_on_unfocus = true;
    config.default_profile.cursor_shape = CursorShapePreference::Underline;
    config.default_profile.cursor_blink = CursorBlinkPreference::On;
    config.default_profile.color_scheme = ColorScheme::monokai();

    // Verify JSON serialization and roundtrip
    let json = config.to_json().expect("to_json should succeed");
    let loaded: AppConfig = AppConfig::from_json(&json).expect("from_json should succeed");
    assert_eq!(config, loaded);
    assert_eq!(loaded.quake_height_percent, 60);
    assert!(loaded.quake_hide_on_unfocus);
    assert_eq!(
        loaded.default_profile.cursor_shape,
        CursorShapePreference::Underline
    );
    assert_eq!(
        loaded.default_profile.cursor_blink,
        CursorBlinkPreference::On
    );
    assert_eq!(loaded.default_profile.color_scheme.name, "Monokai");

    // File IO roundtrip test
    let mut temp_file = std::env::temp_dir();
    temp_file.push(format!("tilix_phase3_cfg_test_{}.json", std::process::id()));
    config.save_to_path(&temp_file).expect("save should succeed");

    let file_loaded = AppConfig::load_from_path(&temp_file).expect("load should succeed");
    assert_eq!(config, file_loaded);
    let _ = std::fs::remove_file(&temp_file);
}
