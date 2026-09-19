#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(deprecated)]

#[path = "../src/model/mod.rs"]
pub mod model;

#[path = "../src/pty/mod.rs"]
pub mod pty;

#[path = "../src/ui/mod.rs"]
pub mod ui;

#[path = "../src/app.rs"]
pub mod app;

use gtk4 as gtk;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use model::config::AppConfig;
use model::profile::Profile;
use model::{PaneTitleStyle, WindowStyle};
use ui::geometry::{
    calculate_window_size_from_cell_size, COMPACT_HEADER_BAR_HEIGHT, COMPACT_PANE_HEADER_HEIGHT,
    COMPACT_TAB_BAR_HEIGHT, DEFAULT_HEADER_BAR_HEIGHT, DEFAULT_PANE_HEADER_HEIGHT,
    DEFAULT_TAB_BAR_HEIGHT,
};
use ui::window::{
    apply_compact_mode_to_all_windows, register_window_instance, run_gtk_test, setup_css,
    TilixWindow,
};

/// 1. Default value of compact_mode in AppConfig must be false.
#[test]
fn test_phase15_compact_mode_default_is_false() {
    let config = AppConfig::default();
    assert!(!config.compact_mode, "Default compact_mode must be false");
}

/// 2. Serde roundtrip preserves compact_mode = true and compact_mode = false.
#[test]
fn test_phase15_config_serde_roundtrip() {
    let mut config = AppConfig::default();
    config.compact_mode = true;
    let json_true = config.to_json().expect("Failed to serialize compact_mode: true");
    let loaded_true = AppConfig::from_json(&json_true).expect("Failed to deserialize compact_mode: true");
    assert!(loaded_true.compact_mode, "compact_mode true should roundtrip");

    config.compact_mode = false;
    let json_false = config.to_json().expect("Failed to serialize compact_mode: false");
    let loaded_false = AppConfig::from_json(&json_false).expect("Failed to deserialize compact_mode: false");
    assert!(!loaded_false.compact_mode, "compact_mode false should roundtrip");
}

/// 3. Backwards compatibility: JSON without compact_mode deserializes to false without error.
#[test]
fn test_phase15_backwards_compatibility_deserialization() {
    let legacy_json = r#"{
        "notifications_enabled": true,
        "default_session_name": "${title}",
        "app_title": "${appName}: ${sessionName}",
        "window_style": "normal"
    }"#;
    let config = AppConfig::from_json(legacy_json).expect("Failed to deserialize legacy json missing compact_mode");
    assert!(!config.compact_mode, "Legacy JSON missing compact_mode should default to false");
}

/// 4. Compact chrome geometry constants integrity.
#[test]
fn test_phase15_geometry_constants_integrity() {
    assert_eq!(COMPACT_HEADER_BAR_HEIGHT, 28);
    assert_eq!(COMPACT_TAB_BAR_HEIGHT, 22);
    assert_eq!(COMPACT_PANE_HEADER_HEIGHT, 22);
}

/// 5. Window geometry calculation: 48px height savings under compact mode for 80x24 profile.
#[test]
fn test_phase15_window_geometry_compact_vs_normal() {
    let profile = Profile::default();
    let normal_cfg = AppConfig {
        compact_mode: false,
        ..Default::default()
    };

    let compact_cfg = AppConfig {
        compact_mode: true,
        ..Default::default()
    };

    let cell_size = Some((9, 20));
    let (nw, nh) = calculate_window_size_from_cell_size(&profile, &normal_cfg, cell_size, None);
    let (cw, ch) = calculate_window_size_from_cell_size(&profile, &compact_cfg, cell_size, None);

    // Normal dimensions: 80 * 9 + 16 (scrollbar) = 736
    // Height: 24 * 20 (cells) + 36 (pane) + 46 (header) + 38 (tab) = 600
    assert_eq!(nw, 736);
    assert_eq!(nh, 600);

    // Compact dimensions: 80 * 9 + 16 = 736
    // Height: 24 * 20 (cells) + 22 (pane) + 28 (header) + 22 (tab) = 552
    assert_eq!(cw, 736);
    assert_eq!(ch, 552);

    // Exact 48px vertical chrome delta
    assert_eq!(nh - ch, 48);
}

/// 6. Window geometry calculation: hidden chrome yields identical dimensions regardless of compact mode.
#[test]
fn test_phase15_window_geometry_compact_with_hidden_chrome() {
    let profile = Profile::default();
    let mut normal_cfg = AppConfig::default();
    normal_cfg.window_style = WindowStyle::HideToolbar;
    normal_cfg.show_tab_bar = false;
    normal_cfg.pane_title_style = PaneTitleStyle::None;
    normal_cfg.compact_mode = false;

    let mut compact_cfg = normal_cfg.clone();
    compact_cfg.compact_mode = true;

    let cell_size = Some((9, 20));
    let (nw, nh) = calculate_window_size_from_cell_size(&profile, &normal_cfg, cell_size, None);
    let (cw, ch) = calculate_window_size_from_cell_size(&profile, &compact_cfg, cell_size, None);

    assert_eq!((nw, nh), (cw, ch));
    assert_eq!(nw, 736);
    assert_eq!(nh, 480);
}

/// 7. CSS stylesheet loading including .compact rules succeeds without error.
#[test]
fn test_phase15_setup_css_loads_compact_rules() {
    run_gtk_test(|| {
        setup_css();
    });
}

/// 8. Reactive projection toggles .compact CSS class across registered window instances.
#[test]
fn test_phase15_reactive_projection_window_css_class() {
    run_gtk_test(|| {
        let app = adw::Application::builder()
            .application_id("com.github.tilix_rust.test_phase15_projection")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        let win = adw::ApplicationWindow::new(&app);
        register_window_instance(&win);

        assert!(!win.has_css_class("compact"));

        apply_compact_mode_to_all_windows(true);
        assert!(win.has_css_class("compact"), "Window should have .compact class when compact mode is true");

        apply_compact_mode_to_all_windows(false);
        assert!(!win.has_css_class("compact"), "Window should not have .compact class when compact mode is false");
    });
}

/// 9. TilixWindow initializes with .compact CSS class when compact_mode is configured.
#[test]
fn test_phase15_tilix_window_compact_mode_initialization() {
    run_gtk_test(|| {
        let mut cfg = AppConfig::load();
        let prev_compact = cfg.compact_mode;
        cfg.compact_mode = true;
        let _ = cfg.save();

        let app = adw::Application::builder()
            .application_id("com.github.tilix_rust.test_phase15_tilix_window")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        let profile = Profile::default();
        let tilix_win = TilixWindow::new_empty_with_profile(&app, &profile);

        assert!(
            tilix_win.window().has_css_class("compact"),
            "TilixWindow should be initialized with .compact CSS class when compact_mode is true"
        );

        // Restore configuration
        let mut restore_cfg = AppConfig::load();
        restore_cfg.compact_mode = prev_compact;
        let _ = restore_cfg.save();
    });
}
