#![allow(dead_code)]
#![allow(unused_imports)]

use std::fs;
use std::path::PathBuf;

#[path = "../src/model/mod.rs"]
mod model;

use model::config::{AppConfig, PaneTitleStyle, WindowStyle};

#[test]
fn test_pane_title_style_default_and_variants() {
    assert_eq!(PaneTitleStyle::default(), PaneTitleStyle::Normal);
    assert_ne!(PaneTitleStyle::Normal, PaneTitleStyle::None);
}

#[test]
fn test_pane_title_style_serde_snake_case() {
    // Serialization
    let normal_json = serde_json::to_string(&PaneTitleStyle::Normal)
        .expect("serialization of PaneTitleStyle::Normal should succeed");
    assert_eq!(normal_json, "\"normal\"");

    let none_json = serde_json::to_string(&PaneTitleStyle::None)
        .expect("serialization of PaneTitleStyle::None should succeed");
    assert_eq!(none_json, "\"none\"");

    // Deserialization
    let normal_deserialized: PaneTitleStyle = serde_json::from_str("\"normal\"")
        .expect("deserialization of \"normal\" should succeed");
    assert_eq!(normal_deserialized, PaneTitleStyle::Normal);

    let none_deserialized: PaneTitleStyle = serde_json::from_str("\"none\"")
        .expect("deserialization of \"none\" should succeed");
    assert_eq!(none_deserialized, PaneTitleStyle::None);

    // Invalid variant returns error
    let invalid_res: Result<PaneTitleStyle, _> = serde_json::from_str("\"hidden\"");
    assert!(invalid_res.is_err());
}

#[test]
fn test_app_config_phase6_defaults() {
    let config = AppConfig::default();
    assert_eq!(config.pane_title_style, PaneTitleStyle::Normal);
    assert!(config.pane_title_show_when_single);
    assert_eq!(config.window_style, WindowStyle::Normal);
    assert!(!config.use_wide_handle);
}

#[test]
fn test_legacy_config_json_v5_deserialization() {
    // Simulates an existing user configuration file written during Phase 5
    // that lacks the new Phase 6 keys (`pane_title_style` and `pane_title_show_when_single`).
    let legacy_v5_json = r#"{
        "window_style": "hide_toolbar",
        "use_wide_handle": true,
        "quake_height_percent": 50,
        "quake_hide_on_unfocus": true,
        "notifications_enabled": true,
        "bell_notifications": false,
        "process_exit_notifications": true
    }"#;

    let config: AppConfig = serde_json::from_str(legacy_v5_json)
        .expect("deserializing legacy v5 json must succeed");

    assert_eq!(config.window_style, WindowStyle::HideToolbar);
    assert!(config.use_wide_handle);
    assert_eq!(config.quake_height_percent, 50);
    // Newly introduced Phase 6 fields must fall back to their defaults
    assert_eq!(config.pane_title_style, PaneTitleStyle::Normal);
    assert!(config.pane_title_show_when_single);
}

#[test]
fn test_config_roundtrip_with_pane_title_settings() {
    let config = AppConfig {
        pane_title_style: PaneTitleStyle::None,
        pane_title_show_when_single: false,
        window_style: WindowStyle::HideToolbar,
        use_wide_handle: true,
        ..Default::default()
    };

    let json_str = config.to_json().expect("to_json should succeed");

    // Verify expected json keys and values
    assert!(json_str.contains("\"pane_title_style\": \"none\""));
    assert!(json_str.contains("\"pane_title_show_when_single\": false"));

    let deserialized = AppConfig::from_json(&json_str)
        .expect("from_json should deserialize correctly");

    assert_eq!(config, deserialized);
    assert_eq!(deserialized.pane_title_style, PaneTitleStyle::None);
    assert!(!deserialized.pane_title_show_when_single);
}

#[test]
fn test_pane_header_visibility_logic_simulation() {
    // Pure function simulating the reactive projection logic in SessionView::update_pane_headers_visibility
    fn evaluate_header_visibility(style: PaneTitleStyle, show_when_single: bool, pane_count: usize) -> bool {
        match style {
            PaneTitleStyle::None => false,
            PaneTitleStyle::Normal => {
                if pane_count <= 1 {
                    show_when_single
                } else {
                    true
                }
            }
        }
    }

    // Normal style, single pane, show_when_single = true -> Visible
    assert!(evaluate_header_visibility(PaneTitleStyle::Normal, true, 1));

    // Normal style, single pane, show_when_single = false -> Hidden
    assert!(!evaluate_header_visibility(PaneTitleStyle::Normal, false, 1));

    // Normal style, multiple panes (2, 3, 4), show_when_single = false -> Always visible
    assert!(evaluate_header_visibility(PaneTitleStyle::Normal, false, 2));
    assert!(evaluate_header_visibility(PaneTitleStyle::Normal, false, 3));
    assert!(evaluate_header_visibility(PaneTitleStyle::Normal, false, 4));

    // Normal style, multiple panes, show_when_single = true -> Visible
    assert!(evaluate_header_visibility(PaneTitleStyle::Normal, true, 2));

    // None style: ALWAYS hidden regardless of show_when_single or pane_count
    assert!(!evaluate_header_visibility(PaneTitleStyle::None, true, 1));
    assert!(!evaluate_header_visibility(PaneTitleStyle::None, false, 1));
    assert!(!evaluate_header_visibility(PaneTitleStyle::None, true, 2));
    assert!(!evaluate_header_visibility(PaneTitleStyle::None, false, 2));
    assert!(!evaluate_header_visibility(PaneTitleStyle::None, true, 5));
    assert!(!evaluate_header_visibility(PaneTitleStyle::None, false, 5));
}

#[test]
fn test_partial_config_updates_pane_title_fields() {
    // Only pane_title_style is specified
    let partial_style_json = r#"{"pane_title_style": "none"}"#;
    let config1: AppConfig = serde_json::from_str(partial_style_json)
        .expect("partial json with pane_title_style must succeed");
    assert_eq!(config1.pane_title_style, PaneTitleStyle::None);
    assert!(config1.pane_title_show_when_single);

    // Only pane_title_show_when_single is specified
    let partial_single_json = r#"{"pane_title_show_when_single": false}"#;
    let config2: AppConfig = serde_json::from_str(partial_single_json)
        .expect("partial json with pane_title_show_when_single must succeed");
    assert_eq!(config2.pane_title_style, PaneTitleStyle::Normal);
    assert!(!config2.pane_title_show_when_single);
}

#[test]
fn test_forward_compatibility_with_unknown_fields() {
    // Config containing future fields from upcoming releases
    let future_json = r#"{
        "pane_title_style": "none",
        "pane_title_show_when_single": false,
        "window_style": "normal",
        "custom_plugin_settings": { "enabled": true }
    }"#;

    let config: AppConfig = serde_json::from_str(future_json)
        .expect("future json with unknown fields must deserialize without error");

    assert_eq!(config.pane_title_style, PaneTitleStyle::None);
    assert!(!config.pane_title_show_when_single);
}

#[test]
fn test_file_roundtrip_io_with_phase6_fields() {
    let mut temp_path = std::env::temp_dir();
    let unique_name = format!("tilix_phase6_test_{}.json", std::process::id());
    temp_path.push(unique_name);

    let config = AppConfig {
        pane_title_style: PaneTitleStyle::None,
        pane_title_show_when_single: false,
        window_style: WindowStyle::HideToolbar,
        use_wide_handle: true,
        ..Default::default()
    };

    config.save_to_path(&temp_path).expect("save_to_path must succeed");

    let loaded = AppConfig::load_from_path(&temp_path).expect("load_from_path must succeed");
    assert_eq!(config, loaded);
    assert_eq!(loaded.pane_title_style, PaneTitleStyle::None);
    assert!(!loaded.pane_title_show_when_single);

    let _ = fs::remove_file(&temp_path);
}
