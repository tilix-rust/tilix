#![allow(dead_code)]
#![allow(unused_imports)]

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

#[path = "../src/model/mod.rs"]
pub mod model;

#[path = "../src/pty/mod.rs"]
pub mod pty;

#[path = "../src/ui/mod.rs"]
pub mod ui;

#[path = "../src/app.rs"]
pub mod app;

use model::config::{AppConfig, ProfileError};
use model::layout::PaneId;
use model::profile::{
    expand_badge_format, expand_title_format, expand_tokens, BadgePosition, CjkWidthPreference,
    CursorBlinkPreference, CursorShapePreference, EraseBindingPreference, ExitActionPreference,
    Profile, ProfileSwitchRule, TerminalBellPreference, TextBlinkModePreference, TitleTokenContext,
};
use model::theme::{ColorScheme, RgbColor};
use pty::shell::{build_spawn_args, format_shell_argv0};
use ui::preferences::TilixPreferencesWindow;
use ui::session_view::SessionView;
use ui::terminal_pane::TerminalPane;
use ui::window::run_gtk_test;
use gtk4::prelude::*;

#[test]
fn test_integration_multi_profile_crud_lifecycle() {
    let mut config = AppConfig::default();

    // 1. Verify initial state
    assert_eq!(config.profiles.len(), 1);
    assert_eq!(config.default_profile_id, "default");
    assert_eq!(config.get_default_profile().name, "Default");

    // 2. Add profile
    let dev_prof = Profile {
        id: "dev".into(),
        name: "Development".into(),
        cell_width_scale: 1.1,
        ..Default::default()
    };
    let dev_id = config.add_profile(dev_prof).expect("add profile should succeed");
    assert_eq!(dev_id, "dev");
    assert_eq!(config.profiles.len(), 2);

    // 3. Duplicate profile
    let cloned = config
        .duplicate_profile("dev")
        .expect("duplicate should succeed");
    assert_eq!(cloned.name, "Development (Copy)");
    assert_eq!(config.profiles.len(), 3);

    // 4. Update profile
    let mut updated_dev = config.get_profile("dev").unwrap().clone();
    updated_dev.badge_text = "DEV".into();
    config.update_profile(updated_dev).expect("update should succeed");
    assert_eq!(config.get_profile("dev").unwrap().badge_text, "DEV");

    // 5. Set default profile
    config.set_default_profile("dev").expect("set default should succeed");
    assert_eq!(config.default_profile_id, "dev");
    assert_eq!(config.default_profile.id, "dev");
    assert_eq!(config.get_default_profile().name, "Development");

    // 6. Delete profile (non-default)
    config.delete_profile(&cloned.id).expect("delete clone should succeed");
    assert_eq!(config.profiles.len(), 2);

    // 7. Delete default profile (should promote remaining profile as default)
    config.delete_profile("dev").expect("delete default should succeed");
    assert_eq!(config.profiles.len(), 1);
    assert_eq!(config.default_profile_id, "default");
    assert_eq!(config.default_profile.id, "default");

    // 8. Cannot delete last remaining profile
    assert_eq!(
        config.delete_profile("default"),
        Err(ProfileError::CannotDeleteLastProfile)
    );
}

#[test]
fn test_integration_legacy_config_backwards_compatibility() {
    // Legacy config lacking profiles and default_profile_id
    let legacy_json = r##"{
        "quake_height_percent": 45,
        "default_profile": {
            "id": "custom-default",
            "name": "Custom Default",
            "color_scheme": {
                "name": "Tilix Dark",
                "comment": "Default theme",
                "foreground-color": "#ffffff",
                "background-color": "#000000",
                "palette": ["#000000", "#111111", "#222222", "#333333", "#444444", "#555555", "#666666", "#777777", "#888888", "#999999", "#aaaaaa", "#bbbbbb", "#cccccc", "#dddddd", "#eeeeee", "#ffffff"]
            },
            "font": "Monospace 14",
            "scrollback_lines": 10000,
            "cursor_shape": "Underline",
            "cursor_blink": "On"
        }
    }"##;

    let config = AppConfig::from_json(legacy_json).expect("legacy JSON should deserialize cleanly");
    assert_eq!(config.quake_height_percent, 45);
    assert_eq!(config.profiles.len(), 1);
    assert_eq!(config.default_profile_id, "custom-default");
    assert_eq!(config.default_profile.id, "custom-default");
    assert_eq!(config.default_profile.font.as_deref(), Some("Monospace 14"));
    assert_eq!(config.default_profile.scrollback_lines, Some(10000));
    assert_eq!(config.default_profile.cursor_shape, CursorShapePreference::Underline);
    assert_eq!(config.default_profile.cursor_blink, CursorBlinkPreference::On);

    // Verify all Phase 9 fields took defaults
    assert_eq!(config.default_profile.terminal_title, "${title}");
    assert_eq!(config.default_profile.cell_width_scale, 1.0);
    assert_eq!(config.default_profile.cell_height_scale, 1.0);
    assert!(config.default_profile.show_scrollbar);
    assert_eq!(config.default_profile.exit_action, ExitActionPreference::Close);
    assert_eq!(config.default_profile.badge_position, BadgePosition::Northeast);
    assert!(config.default_profile.automatic_switch.is_empty());
}

#[test]
fn test_integration_token_expansion_engine() {
    let ctx = TitleTokenContext {
        id: 99,
        title: "cargo test",
        profile_name: "RustDev",
        directory: Some(Path::new("/workspace/project")),
        app_name: "Tilix",
    };

    let title_res = expand_title_format(
        "[${appName}] ${profile}: ${title} (${id}) in ${directory}",
        &ctx,
    );
    assert_eq!(
        title_res,
        "[Tilix] RustDev: cargo test (99) in /workspace/project"
    );

    let badge_res = expand_badge_format("${profile} #${id}", &ctx);
    assert_eq!(badge_res, "RustDev #99");

    // Test missing directory fallback
    let ctx_no_dir = TitleTokenContext {
        id: 1,
        title: "bash",
        profile_name: "Default",
        directory: None,
        app_name: "",
    };
    let no_dir_res = expand_tokens("${appName}-${profile}-${directory}", &ctx_no_dir);
    assert_eq!(no_dir_res, "Tilix-Default-");
}

#[test]
fn test_integration_automatic_profile_switch_rule_matching() {
    let rule = ProfileSwitchRule {
        hostname: "build-box".into(),
        directory: "/var/builds".into(),
        profile_id: "builder".into(),
    };

    // Exact hostname & directory
    assert!(rule.matches("build-box", Path::new("/var/builds")));
    // Case-insensitive hostname
    assert!(rule.matches("BUILD-BOX", Path::new("/var/builds")));
    // Directory subdirectory prefix
    assert!(rule.matches("build-box", Path::new("/var/builds/tilix")));
    // Mismatched hostname
    assert!(!rule.matches("dev-box", Path::new("/var/builds")));
    // Mismatched directory
    assert!(!rule.matches("build-box", Path::new("/home/user")));
    // Avoid false prefix match
    assert!(!rule.matches("build-box", Path::new("/var/builds_other")));

    // Wildcard hostname
    let wildcard = ProfileSwitchRule {
        hostname: "*".into(),
        directory: "/opt".into(),
        profile_id: "opt-prof".into(),
    };
    assert!(wildcard.matches("any-host", Path::new("/opt/app")));
    assert!(!wildcard.matches("any-host", Path::new("/etc")));
}

#[test]
fn test_integration_pty_shell_args_construction() {
    // Login shell argv0 formatting
    assert_eq!(format_shell_argv0("/bin/bash", true), "-bash");
    assert_eq!(format_shell_argv0("/usr/bin/zsh", true), "-zsh");
    assert_eq!(format_shell_argv0("/bin/sh", false), "sh");

    // Default shell spawn args
    let mut profile = Profile {
        login_shell: true,
        ..Default::default()
    };
    let (cmd, args) = build_spawn_args(&profile, "/bin/zsh");
    assert_eq!(cmd, "/bin/zsh");
    assert_eq!(args, vec!["-zsh"]);

    // Custom command spawn args
    profile.use_custom_command = true;
    profile.custom_command = "btop --utf-force".into();
    let (custom_cmd, custom_args) = build_spawn_args(&profile, "/bin/zsh");
    assert_eq!(custom_cmd, "/bin/sh");
    assert_eq!(custom_args, vec!["/bin/sh", "-c", "btop --utf-force"]);

    // Empty custom command fallback
    profile.custom_command = "  ".into();
    let (fb_cmd, fb_args) = build_spawn_args(&profile, "/bin/zsh");
    assert_eq!(fb_cmd, "/bin/zsh");
    assert_eq!(fb_args, vec!["-zsh"]);
}

#[test]
fn test_integration_terminal_pane_profile_properties_and_exit_action() {
    run_gtk_test(|| {
        let pane = TerminalPane::new(PaneId(1), None);

        let prof = Profile {
            show_scrollbar: false,
            badge_text: "BADGE ${id}".into(),
            badge_position: BadgePosition::Southeast,
            draw_margin: 100,
            dim_transparency_percent: 20,
            exit_action: ExitActionPreference::Hold,
            cell_width_scale: 1.15,
            cell_height_scale: 1.05,
            ..Default::default()
        };

        pane.apply_profile(&prof);

        // Verify scrollbar hidden
        assert!(!pane.is_scrollbar_visible());

        // Verify badge
        let badge = pane.badge_label();
        assert!(badge.is_visible());
        assert_eq!(badge.text().as_str(), "BADGE 1");
        assert_eq!(badge.halign(), gtk4::Align::End);
        assert_eq!(badge.valign(), gtk4::Align::End);
        assert!(!badge.can_target());

        // Verify margin line
        assert!(pane.margin_line().is_visible());

        // Verify inactive dimming
        pane.set_active(false);
        assert!((pane.terminal().opacity() - 0.8).abs() < 0.01);
        pane.set_active(true);
        assert_eq!(pane.terminal().opacity(), 1.0);

        // Verify ExitActionPreference::Hold preserves pane and appends status to title
        let close_called = Rc::new(std::cell::Cell::new(false));
        let cc = Rc::clone(&close_called);
        pane.connect_close(move |_| {
            cc.set(true);
        });

        use glib::prelude::ObjectExt;
        pane.terminal().emit_by_name::<()>("child-exited", &[&127i32]);

        assert!(!close_called.get());
        assert!(pane.title().contains("[Process exited: 127]"));
    });
}

#[test]
fn test_integration_session_view_per_pane_profile() {
    run_gtk_test(|| {
        let session = SessionView::new();
        assert_eq!(session.pane_count(), 1);

        let pane_id = PaneId(1);
        let current_prof = session.get_pane_profile(pane_id);
        assert!(current_prof.is_some());
        assert_eq!(current_prof.unwrap().name, "Default");

        let custom = Profile {
            id: "custom-pane".into(),
            name: "Custom Pane Profile".into(),
            badge_text: "PANE 1".into(),
            ..Default::default()
        };

        session.apply_profile_to_pane(pane_id, &custom);
        let updated_prof = session.get_pane_profile(pane_id).unwrap();
        assert_eq!(updated_prof.name, "Custom Pane Profile");
        assert_eq!(updated_prof.badge_text, "PANE 1");
    });
}

#[test]
fn test_integration_preferences_window_profiles_page() {
    run_gtk_test(|| {
        let profile_sync_called = Rc::new(RefCell::new(false));
        let psc = Rc::clone(&profile_sync_called);

        let pref_win = TilixPreferencesWindow::new(None::<&gtk4::Window>, move |_p| {
            *psc.borrow_mut() = true;
        });

        assert_eq!(pref_win.window().title().as_deref(), Some("Preferences"));
    });
}
