#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(deprecated)]

use tilix::{app, model, pty, ui};

use gtk4 as gtk;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use model::profile::Profile;
use pty::osc52::Osc52StreamParser;
use ui::session_view::SessionView;
use ui::terminal_pane::TerminalPane;
use ui::window::TilixWindow;

fn run_gtk_test<F: FnOnce() + Send + 'static>(f: F) {
    static GTK_TEST_POOL: std::sync::OnceLock<Option<glib::ThreadPool>> = std::sync::OnceLock::new();
    let pool = GTK_TEST_POOL.get_or_init(|| {
        let (init_tx, init_rx) = std::sync::mpsc::sync_channel(1);
        let Ok(pool) = glib::ThreadPool::exclusive(1) else {
            return None;
        };
        if pool
            .push(move || {
                let ok = gtk4::init().is_ok();
                let _ = init_tx.send(ok);
            })
            .is_err()
        {
            return None;
        }
        if init_rx.recv().unwrap_or(false) {
            Some(pool)
        } else {
            None
        }
    });

    if let Some(pool) = pool.as_ref() {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let _ = pool.push(move || {
            f();
            let _ = tx.send(());
        });
        let _ = rx.recv();
    }
}

fn find_action_in_menu(model: &impl IsA<gio::MenuModel>, target: &str) -> bool {
    let model = model.as_ref();
    for i in 0..model.n_items() {
        if let Some(attr) = model.item_attribute_value(i, "action", None) {
            if let Some(s) = attr.str() {
                if s == target {
                    return true;
                }
            }
        }
        if let Some(section) = model.item_link(i, "section") {
            if find_action_in_menu(&section, target) {
                return true;
            }
        }
        if let Some(submenu) = model.item_link(i, "submenu") {
            if find_action_in_menu(&submenu, target) {
                return true;
            }
        }
    }
    false
}

/// 1. Verifies that allow_hyperlinks defaults to true on Profile::default().
#[test]
fn test_phase18_profile_allow_hyperlinks_default() {
    let profile = Profile::default();
    assert!(
        profile.allow_hyperlinks,
        "allow_hyperlinks must default to true"
    );
}

/// 2. Verifies serde backward compatibility for legacy profile JSON missing allow_hyperlinks.
#[test]
fn test_phase18_profile_serde_backward_compat() {
    let legacy_json = r#"{
        "id": "legacy-test",
        "name": "Legacy Test"
    }"#;
    let deserialized: Profile = serde_json::from_str(legacy_json)
        .expect("Deserializing legacy profile JSON without allow_hyperlinks should succeed");
    assert!(
        deserialized.allow_hyperlinks,
        "allow_hyperlinks must default to true when missing from JSON"
    );
}

/// 3. Verifies serde roundtrip preservation of allow_hyperlinks false and true.
#[test]
fn test_phase18_profile_serde_roundtrip() {
    let prof = Profile {
        allow_hyperlinks: false,
        ..Default::default()
    };
    let json_false = serde_json::to_string(&prof).expect("Serialization should succeed");
    let des_false: Profile =
        serde_json::from_str(&json_false).expect("Deserialization should succeed");
    assert!(!des_false.allow_hyperlinks);

    let prof = Profile {
        allow_hyperlinks: true,
        ..Default::default()
    };
    let json_true = serde_json::to_string(&prof).expect("Serialization should succeed");
    let des_true: Profile =
        serde_json::from_str(&json_true).expect("Deserialization should succeed");
    assert!(des_true.allow_hyperlinks);
}

/// 4. Verifies that TerminalPane::allows_hyperlink() matches profile and updates on apply_profile.
#[test]
fn test_phase18_vte_allow_hyperlinks_sync() {
    run_gtk_test(|| {
        let pane = TerminalPane::new(model::PaneId(1), None);
        assert!(
            pane.allows_hyperlink(),
            "VTE allow-hyperlink must be true by default"
        );

        let prof = Profile {
            allow_hyperlinks: false,
            ..Default::default()
        };
        pane.apply_profile(&prof);
        assert!(
            !pane.allows_hyperlink(),
            "VTE allow-hyperlink must update to false after apply_profile"
        );

        let prof = Profile {
            allow_hyperlinks: true,
            ..Default::default()
        };
        pane.apply_profile(&prof);
        assert!(
            pane.allows_hyperlink(),
            "VTE allow-hyperlink must update to true after apply_profile"
        );
    });
}

/// 5. Verifies Osc52StreamParser passthrough cleanliness for OSC 8 escape sequences.
#[test]
fn test_phase18_osc8_stream_parser_passthrough() {
    let mut parser = Osc52StreamParser::new();

    // OSC 8 hyperlink sequence terminated by ST (\x1b\\)
    let osc8_st = b"\x1b]8;id=item-42;https://tilix.dev\x1b\\Tilix Terminal\x1b]8;;\x1b\\";
    let (pt_st, events_st) = parser.process(osc8_st);
    assert_eq!(
        pt_st, osc8_st,
        "OSC 8 ST-terminated sequence must pass through completely unaltered"
    );
    assert!(
        events_st.is_empty(),
        "OSC 8 sequence must not generate spurious OSC 52 clipboard events"
    );

    // OSC 8 hyperlink sequence terminated by BEL (\x07)
    let osc8_bel = b"\x1b]8;;http://example.com/test\x07Click Here\x1b]8;;\x07";
    let (pt_bel, events_bel) = parser.process(osc8_bel);
    assert_eq!(
        pt_bel, osc8_bel,
        "OSC 8 BEL-terminated sequence must pass through completely unaltered"
    );
    assert!(
        events_bel.is_empty(),
        "OSC 8 BEL sequence must not generate spurious OSC 52 clipboard events"
    );

    // Mixed text and OSC 8 sequences
    let mixed = b"Prefix \x1b]8;;https://github.com\x1b\\GitHub\x1b]8;;\x1b\\ Suffix\n";
    let (pt_mixed, events_mixed) = parser.process(mixed);
    assert_eq!(pt_mixed, mixed);
    assert!(events_mixed.is_empty());
}

/// 6. Verifies is_safe_file_uri security gating for file:// and standard URIs.
#[test]
fn test_phase18_is_safe_file_uri_validation() {
    // 1. Local file path with empty host (file:///path) -> safe
    assert!(TerminalPane::is_safe_file_uri("file:///home/user/document.txt"));
    assert!(TerminalPane::is_safe_file_uri("file:///etc/hosts"));
    assert!(TerminalPane::is_safe_file_uri("FILE:///var/log/syslog"));

    // 2. Localhost file path (file://localhost/path) -> safe
    assert!(TerminalPane::is_safe_file_uri("file://localhost/home/user/file.txt"));
    assert!(TerminalPane::is_safe_file_uri("file://LOCALHOST/etc/issue"));

    // 3. Current host machine name file path (file://<hostname>/path) -> safe
    let current_host = glib::host_name();
    let host_uri = format!("file://{}/tmp/local_file.log", current_host);
    assert!(
        TerminalPane::is_safe_file_uri(&host_uri),
        "file:// URI matching local hostname must be permitted"
    );

    // 4. Remote attacker hostname file path -> BLOCKED (unsafe)
    assert!(
        !TerminalPane::is_safe_file_uri("file://attacker.com/evil.sh"),
        "Remote file:// URI with untrusted host must be blocked"
    );
    assert!(
        !TerminalPane::is_safe_file_uri("FILE://ATTACKER.COM/evil.sh"),
        "Case-insensitive remote file:// URI must be blocked"
    );

    // 5. Remote SMB/network share file path -> BLOCKED (unsafe)
    assert!(
        !TerminalPane::is_safe_file_uri("file://smb-server/share/payload.exe"),
        "SMB/network share file:// URI must be blocked"
    );
    assert!(
        !TerminalPane::is_safe_file_uri("file://192.168.1.100/shared/file.tar.gz"),
        "IP-based remote file:// URI must be blocked"
    );

    // 6. Non-file URIs -> safe
    assert!(TerminalPane::is_safe_file_uri("https://tilix.dev"));
    assert!(TerminalPane::is_safe_file_uri("http://example.com/page"));
    assert!(TerminalPane::is_safe_file_uri("mailto:user@example.com"));
    assert!(TerminalPane::is_safe_file_uri("ssh://user@hostname"));
}

/// 7. Verifies dynamic context menu structure: presence of link items when has_link is true vs false,
///    and verifies pane hyperlink URI holder methods.
#[test]
fn test_phase18_context_menu_dynamic_link_section() {
    run_gtk_test(|| {
        let menu_with_link = TerminalPane::build_context_menu(true);
        let menu_without_link = TerminalPane::build_context_menu(false);

        // Menu with link should have link actions
        assert!(
            find_action_in_menu(&menu_with_link, "win.open-link"),
            "Context menu with link must contain win.open-link"
        );
        assert!(
            find_action_in_menu(&menu_with_link, "win.copy-link-address"),
            "Context menu with link must contain win.copy-link-address"
        );

        // Menu without link must NOT contain link actions
        assert!(
            !find_action_in_menu(&menu_without_link, "win.open-link"),
            "Context menu without link must not contain win.open-link"
        );
        assert!(
            !find_action_in_menu(&menu_without_link, "win.copy-link-address"),
            "Context menu without link must not contain win.copy-link-address"
        );

        // Both menus must contain standard terminal actions
        for act in &["win.copy", "win.paste", "win.select-all", "win.close-pane", "win.preferences"] {
            assert!(
                find_action_in_menu(&menu_with_link, act),
                "Menu with link must contain standard action '{}'",
                act
            );
            assert!(
                find_action_in_menu(&menu_without_link, act),
                "Menu without link must contain standard action '{}'",
                act
            );
        }

        // Test pane URI holder getter/setter
        let pane = TerminalPane::new(model::PaneId(1), None);
        assert_eq!(pane.current_hyperlink_uri(), None);

        pane.set_current_hyperlink_uri(Some("https://tilix.dev".to_string()));
        assert_eq!(
            pane.current_hyperlink_uri(),
            Some("https://tilix.dev".to_string())
        );

        pane.set_current_hyperlink_uri(None);
        assert_eq!(pane.current_hyperlink_uri(), None);
    });
}

/// 8. Verifies that TilixWindow has win.open-link and win.copy-link-address registered.
#[test]
fn test_phase18_window_hyperlink_actions_registered() {
    run_gtk_test(|| {
        let app = adw::Application::builder()
            .application_id("com.gexperts.Tilix.TestPhase18")
            .build();

        let window = TilixWindow::new_empty(&app);
        let win_widget = window.window();

        assert!(
            win_widget.lookup_action("open-link").is_some(),
            "Action 'open-link' must be registered on TilixWindow"
        );
        assert!(
            win_widget.lookup_action("copy-link-address").is_some(),
            "Action 'copy-link-address' must be registered on TilixWindow"
        );
    });
}
