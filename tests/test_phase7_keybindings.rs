#![allow(dead_code)]
#![allow(unused_imports)]

use std::fs;
use std::path::PathBuf;

#[path = "../src/model/mod.rs"]
mod model;

use model::config::{AppConfig, PaneTitleStyle, WindowStyle};
use model::keybindings::{
    normalize_accelerator, ActionCategory, ActionShortcutDef, ConflictInfo, KeybindingsConfig,
    ACTION_CATALOG,
};

#[test]
fn test_action_catalog_integrity_and_default_uniqueness() {
    assert_eq!(ACTION_CATALOG.len(), 27);

    let mut ids = std::collections::HashSet::new();
    let mut default_accel_map: std::collections::HashMap<String, &'static str> =
        std::collections::HashMap::new();

    for def in ACTION_CATALOG {
        assert!(!def.id.is_empty(), "Action ID must not be empty");
        assert!(!def.title.is_empty(), "Action title must not be empty");
        assert!(
            !def.description.is_empty(),
            "Action description must not be empty"
        );
        assert!(
            ids.insert(def.id),
            "Duplicate action ID detected: {}",
            def.id
        );

        for accel in def.default_accels {
            let norm = normalize_accelerator(accel);
            if !norm.is_empty() {
                if let Some(existing_action) = default_accel_map.get(&norm) {
                    panic!(
                        "Default accelerator collision: '{}' is used by both '{}' and '{}'",
                        norm, existing_action, def.id
                    );
                }
                default_accel_map.insert(norm, def.id);
            }
        }
    }
}

#[test]
fn test_action_categories_and_titles() {
    assert_eq!(ActionCategory::SessionAndTabs.title(), "Session & Tabs");
    assert_eq!(ActionCategory::SplitsAndLayout.title(), "Splits & Layout");
    assert_eq!(ActionCategory::Navigation.title(), "Navigation");
    assert_eq!(ActionCategory::ViewAndSettings.title(), "View & Settings");

    let session_actions: Vec<_> = ACTION_CATALOG
        .iter()
        .filter(|d| d.category == ActionCategory::SessionAndTabs)
        .collect();
    let split_actions: Vec<_> = ACTION_CATALOG
        .iter()
        .filter(|d| d.category == ActionCategory::SplitsAndLayout)
        .collect();
    let nav_actions: Vec<_> = ACTION_CATALOG
        .iter()
        .filter(|d| d.category == ActionCategory::Navigation)
        .collect();
    let view_actions: Vec<_> = ACTION_CATALOG
        .iter()
        .filter(|d| d.category == ActionCategory::ViewAndSettings)
        .collect();

    assert_eq!(session_actions.len(), 14);
    assert_eq!(split_actions.len(), 4);
    assert_eq!(nav_actions.len(), 4);
    assert_eq!(view_actions.len(), 5);
}

#[test]
fn test_legacy_config_v1_to_v6_deserialization() {
    // Phase 1 minimal payload
    let v1_json = r#"{
        "quake_height_percent": 30,
        "quake_hide_on_unfocus": false
    }"#;
    let v1_cfg: AppConfig =
        serde_json::from_str(v1_json).expect("v1 json deserialization must succeed");
    assert!(v1_cfg.keybindings.custom.is_empty());
    assert_eq!(
        v1_cfg.keybindings.get_effective_accel("win.new-tab"),
        Some("<Primary><Shift>t".to_string())
    );

    // Phase 5 payload
    let v5_json = r#"{
        "window_style": "hide_toolbar",
        "use_wide_handle": true
    }"#;
    let v5_cfg: AppConfig =
        serde_json::from_str(v5_json).expect("v5 json deserialization must succeed");
    assert_eq!(v5_cfg.window_style, WindowStyle::HideToolbar);
    assert!(v5_cfg.use_wide_handle);
    assert!(v5_cfg.keybindings.custom.is_empty());

    // Phase 6 payload
    let v6_json = r#"{
        "pane_title_style": "none",
        "pane_title_show_when_single": false,
        "show_tab_bar": false
    }"#;
    let v6_cfg: AppConfig =
        serde_json::from_str(v6_json).expect("v6 json deserialization must succeed");
    assert_eq!(v6_cfg.pane_title_style, PaneTitleStyle::None);
    assert!(!v6_cfg.pane_title_show_when_single);
    assert!(!v6_cfg.show_tab_bar);
    assert!(v6_cfg.keybindings.custom.is_empty());
    assert_eq!(
        v6_cfg.keybindings.get_effective_accel("win.toggle-tab-bar"),
        Some("F12".to_string())
    );
}

#[test]
fn test_app_config_full_json_roundtrip_with_custom_keybindings() {
    let mut config = AppConfig {
        window_style: WindowStyle::HideToolbar,
        use_wide_handle: true,
        pane_title_style: PaneTitleStyle::None,
        pane_title_show_when_single: false,
        show_tab_bar: false,
        ..Default::default()
    };

    config
        .keybindings
        .set_custom_accel("win.new-tab", "<Primary>t");
    config
        .keybindings
        .set_custom_accel("win.close-pane", "<Primary>w");
    config
        .keybindings
        .set_custom_accel("win.toggle-sync-input", "");

    let json_str = config.to_json().expect("to_json should succeed");
    assert!(json_str.contains("\"win.new-tab\": \"<Primary>t\""));
    assert!(json_str.contains("\"win.close-pane\": \"<Primary>w\""));
    assert!(json_str.contains("\"win.toggle-sync-input\": \"\""));

    let deserialized = AppConfig::from_json(&json_str).expect("from_json should succeed");
    assert_eq!(config, deserialized);
    assert_eq!(
        deserialized.keybindings.get_effective_accel("win.new-tab"),
        Some("<Primary>t".to_string())
    );
    assert_eq!(
        deserialized.keybindings.get_effective_accel("win.close-pane"),
        Some("<Primary>w".to_string())
    );
    assert_eq!(
        deserialized
            .keybindings
            .get_effective_accel("win.toggle-sync-input"),
        Some("".to_string())
    );
    assert_eq!(
        deserialized
            .keybindings
            .get_all_effective_accels("win.toggle-sync-input"),
        Vec::<String>::new()
    );
}

#[test]
fn test_keybindings_file_roundtrip_io() {
    let mut temp_path = std::env::temp_dir();
    let unique_name = format!("tilix_test_phase7_kb_{}.json", std::process::id());
    temp_path.push(unique_name);

    let mut config = AppConfig::default();
    config
        .keybindings
        .set_custom_accel("win.split-right", "<Primary>e");
    config
        .keybindings
        .set_custom_accel("win.split-down", "<Primary>o");
    config.save_to_path(&temp_path).expect("save must succeed");

    let loaded = AppConfig::load_from_path(&temp_path).expect("load must succeed");
    assert_eq!(config, loaded);
    assert_eq!(
        loaded.keybindings.get_effective_accel("win.split-right"),
        Some("<Primary>e".to_string())
    );
    assert_eq!(
        loaded.keybindings.get_effective_accel("win.split-down"),
        Some("<Primary>o".to_string())
    );

    let _ = fs::remove_file(&temp_path);
}

#[test]
fn test_normalization_and_conflict_permutations() {
    // Normalization permutations
    assert_eq!(
        normalize_accelerator("<Shift><Primary>t"),
        "<Primary><Shift>t"
    );
    assert_eq!(
        normalize_accelerator("<Control><Shift>t"),
        "<Primary><Shift>t"
    );
    assert_eq!(
        normalize_accelerator("<ctrl><shift>t"),
        "<Primary><Shift>t"
    );
    assert_eq!(
        normalize_accelerator("<primary><shift>t"),
        "<Primary><Shift>t"
    );
    assert_eq!(
        normalize_accelerator("<ctrl><alt>up"),
        "<Primary><Alt>Up"
    );
    assert_eq!(
        normalize_accelerator("<super><primary><shift>F12"),
        "<Primary><Shift><Super>F12"
    );
    assert_eq!(
        normalize_accelerator("<alt>1"),
        "<Alt>1"
    );
    assert_eq!(
        normalize_accelerator("  <Primary>comma  "),
        "<Primary>comma"
    );

    // Conflict detection
    let mut config = KeybindingsConfig::default();

    // Collision with default accelerator of another action
    let conflict = config.check_conflict("win.split-right", "<Control><Shift>t");
    assert!(conflict.is_some());
    let c = conflict.unwrap();
    assert_eq!(c.action_id, "win.new-tab");
    assert_eq!(c.conflicting_accel, "<Primary><Shift>t");

    // Self-conflict ignored
    assert!(config
        .check_conflict("win.new-tab", "<Primary><Shift>t")
        .is_none());

    // Empty or disabled candidate never conflicts
    assert!(config.check_conflict("win.new-tab", "").is_none());
    assert!(config.check_conflict("win.new-tab", "   ").is_none());

    // Custom override conflict
    config.set_custom_accel("win.preferences", "<Primary>p");
    let pref_conflict = config.check_conflict("win.new-tab", "<Primary>p");
    assert!(pref_conflict.is_some());
    assert_eq!(pref_conflict.unwrap().action_id, "win.preferences");
}

#[test]
fn test_forward_compatibility_with_unknown_fields() {
    let forward_json = r#"{
        "unknown_future_field": "future_value",
        "keybindings": {
            "custom": {
                "win.new-tab": "<Primary>t"
            },
            "future_kb_setting": 123
        }
    }"#;

    let config: AppConfig = serde_json::from_str(forward_json)
        .expect("deserializing forward compatible json must succeed");
    assert_eq!(
        config.keybindings.get_effective_accel("win.new-tab"),
        Some("<Primary>t".to_string())
    );
}

#[test]
fn test_minimal_footprint_reverting_to_default() {
    let mut config = KeybindingsConfig::default();
    assert!(!config.is_customized("win.new-tab"));

    // Set to custom
    config.set_custom_accel("win.new-tab", "<Primary>t");
    assert!(config.is_customized("win.new-tab"));
    assert_eq!(config.custom.len(), 1);

    // Revert to exact default string
    config.set_custom_accel("win.new-tab", "<Primary><Shift>t");
    assert!(!config.is_customized("win.new-tab"));
    assert!(config.custom.is_empty());

    // Revert via alias
    config.set_custom_accel("win.new-tab", "<Primary>t");
    assert!(config.is_customized("win.new-tab"));
    config.set_custom_accel("win.new-tab", "<Shift><Control>t");
    assert!(!config.is_customized("win.new-tab"));
    assert!(config.custom.is_empty());
}
