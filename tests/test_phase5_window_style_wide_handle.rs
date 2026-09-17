#![allow(dead_code)]
#![allow(unused_imports)]

use std::fs;
use std::path::PathBuf;

#[path = "../src/model/mod.rs"]
mod model;

use model::config::{AppConfig, WindowStyle};

#[test]
fn test_window_style_default_and_variants() {
    assert_eq!(WindowStyle::default(), WindowStyle::Normal);
    assert_ne!(WindowStyle::Normal, WindowStyle::HideToolbar);
}

#[test]
fn test_window_style_serde_snake_case_representations() {
    // Serialization
    let normal_json = serde_json::to_string(&WindowStyle::Normal)
        .expect("serialization of WindowStyle::Normal should succeed");
    assert_eq!(normal_json, "\"normal\"");

    let hide_json = serde_json::to_string(&WindowStyle::HideToolbar)
        .expect("serialization of WindowStyle::HideToolbar should succeed");
    assert_eq!(hide_json, "\"hide_toolbar\"");

    // Deserialization
    let normal_deserialized: WindowStyle = serde_json::from_str("\"normal\"")
        .expect("deserialization of \"normal\" should succeed");
    assert_eq!(normal_deserialized, WindowStyle::Normal);

    let hide_deserialized: WindowStyle = serde_json::from_str("\"hide_toolbar\"")
        .expect("deserialization of \"hide_toolbar\" should succeed");
    assert_eq!(hide_deserialized, WindowStyle::HideToolbar);

    // Invalid variant returns error
    let invalid_res: Result<WindowStyle, _> = serde_json::from_str("\"invalid_style\"");
    assert!(invalid_res.is_err());
}

#[test]
fn test_app_config_phase5_defaults() {
    let config = AppConfig::default();
    assert_eq!(config.window_style, WindowStyle::Normal);
    assert!(!config.use_wide_handle);
    assert_eq!(config.quake_height_percent, 40);
    assert!(!config.quake_hide_on_unfocus);
    assert!(config.notifications_enabled);
    assert!(config.bell_notifications);
    assert!(config.process_exit_notifications);
}

#[test]
fn test_legacy_v4_json_deserialization_defaults() {
    // Simulates an existing user configuration file written during Phase 1-4
    // that lacks the new Phase 5 keys (`window_style` and `use_wide_handle`).
    let legacy_v4_json = r#"{
        "quake_height_percent": 60,
        "quake_hide_on_unfocus": true,
        "notifications_enabled": true,
        "bell_notifications": false,
        "process_exit_notifications": true
    }"#;

    let config: AppConfig = serde_json::from_str(legacy_v4_json)
        .expect("deserializing legacy v4 json must succeed");

    assert_eq!(config.quake_height_percent, 60);
    assert!(config.quake_hide_on_unfocus);
    assert!(!config.bell_notifications);
    // Newly introduced Phase 5 fields must fall back to their defaults
    assert_eq!(config.window_style, WindowStyle::Normal);
    assert!(!config.use_wide_handle);
}

#[test]
fn test_roundtrip_serialization_phase5_custom_settings() {
    let config = AppConfig {
        window_style: WindowStyle::HideToolbar,
        use_wide_handle: true,
        ..Default::default()
    };

    let json_str = config.to_json().expect("to_json should succeed");

    // Verify expected json keys and values
    assert!(json_str.contains("\"window_style\": \"hide_toolbar\""));
    assert!(json_str.contains("\"use_wide_handle\": true"));

    let deserialized = AppConfig::from_json(&json_str)
        .expect("from_json should deserialize correctly");

    assert_eq!(config, deserialized);
    assert_eq!(deserialized.window_style, WindowStyle::HideToolbar);
    assert!(deserialized.use_wide_handle);
}

#[test]
fn test_partial_config_updates_window_style_and_wide_handle() {
    // Only window_style is specified
    let partial_style_json = r#"{"window_style": "hide_toolbar"}"#;
    let config1: AppConfig = serde_json::from_str(partial_style_json)
        .expect("partial json with window_style must succeed");
    assert_eq!(config1.window_style, WindowStyle::HideToolbar);
    assert!(!config1.use_wide_handle);

    // Only use_wide_handle is specified
    let partial_handle_json = r#"{"use_wide_handle": true}"#;
    let config2: AppConfig = serde_json::from_str(partial_handle_json)
        .expect("partial json with use_wide_handle must succeed");
    assert_eq!(config2.window_style, WindowStyle::Normal);
    assert!(config2.use_wide_handle);
}

#[test]
fn test_forward_compatibility_with_unknown_fields() {
    // Config containing future fields from upcoming releases
    let future_json = r#"{
        "window_style": "hide_toolbar",
        "use_wide_handle": true,
        "experimental_gpu_acceleration": true,
        "custom_plugin_settings": { "enabled": true }
    }"#;

    let config: AppConfig = serde_json::from_str(future_json)
        .expect("future json with unknown fields must deserialize without error");

    assert_eq!(config.window_style, WindowStyle::HideToolbar);
    assert!(config.use_wide_handle);
}

#[test]
fn test_file_roundtrip_io_with_phase5_fields() {
    let mut temp_path = std::env::temp_dir();
    let unique_name = format!("tilix_phase5_test_{}.json", std::process::id());
    temp_path.push(unique_name);

    let config = AppConfig {
        window_style: WindowStyle::HideToolbar,
        use_wide_handle: true,
        ..Default::default()
    };

    config.save_to_path(&temp_path).expect("save_to_path must succeed");

    let loaded = AppConfig::load_from_path(&temp_path).expect("load_from_path must succeed");
    assert_eq!(config, loaded);
    assert_eq!(loaded.window_style, WindowStyle::HideToolbar);
    assert!(loaded.use_wide_handle);

    let _ = fs::remove_file(&temp_path);
}
