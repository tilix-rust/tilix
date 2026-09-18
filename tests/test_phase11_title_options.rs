#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(deprecated)]

use std::cell::RefCell;
use std::rc::Rc;

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
use model::title::{expand_title_tokens, expand_title_tokens_scoped, TitleEditScope, TokenContext};
use ui::preferences::{create_scoped_token_menu_button, TilixPreferencesWindow};
use ui::window::{run_gtk_test, TilixWindow};

fn find_widgets<T: IsA<gtk::Widget>>(root: &impl IsA<gtk::Widget>) -> Vec<T> {
    let mut found = Vec::new();
    let root_w = root.as_ref();
    let mut child = root_w.first_child();
    while let Some(w) = child {
        if let Ok(matched) = w.clone().downcast::<T>() {
            found.push(matched);
        }
        found.extend(find_widgets::<T>(&w));
        child = w.next_sibling();
    }
    found
}

fn find_button_by_label(root: &impl IsA<gtk::Widget>, label_text: &str) -> Option<gtk::Button> {
    for btn in find_widgets::<gtk::Button>(root) {
        if let Some(lbl) = btn.label() {
            if lbl.as_str() == label_text {
                return Some(btn);
            }
        }
    }
    None
}

fn find_label_by_text(root: &impl IsA<gtk::Widget>, text: &str) -> Option<gtk::Label> {
    for lbl in find_widgets::<gtk::Label>(root) {
        let t = lbl.text();
        if t.as_str() == text || t.as_str().contains(text) {
            return Some(lbl);
        }
    }
    None
}

/// 1. AppConfig default values and JSON serde backwards/forwards compatibility.
#[test]
fn test_phase11_app_config_title_defaults_and_serde() {
    let cfg = AppConfig::default();
    assert_eq!(cfg.default_session_name, "${title}");
    assert_eq!(cfg.app_title, "${appName}: ${sessionName}");

    // JSON round-trip
    let json = serde_json::to_string(&cfg).expect("serialize config to json");
    let deserialized: AppConfig = serde_json::from_str(&json).expect("deserialize config from json");
    assert_eq!(deserialized.default_session_name, "${title}");
    assert_eq!(deserialized.app_title, "${appName}: ${sessionName}");

    // Legacy JSON without title fields
    let legacy_json = r#"{
        "notifications_enabled": false
    }"#;
    let legacy_cfg: AppConfig = serde_json::from_str(legacy_json).expect("deserialize legacy json");
    assert_eq!(legacy_cfg.default_session_name, "${title}");
    assert_eq!(legacy_cfg.app_title, "${appName}: ${sessionName}");
}

/// 2. Evaluation of all 12 Terminal tokens + profile.
#[test]
fn test_phase11_terminal_token_catalog() {
    let mut ctx = TokenContext::new_terminal("my-term");
    ctx.icon_title = Some("my-icon".to_string());
    ctx.id = Some(42);
    ctx.directory = Some(std::path::PathBuf::from("/home/tilix/src"));
    ctx.hostname = Some("my-host".to_string());
    ctx.username = Some("testuser".to_string());
    ctx.columns = Some(120);
    ctx.rows = Some(40);
    ctx.process = Some("zsh".to_string());
    ctx.readonly = true;
    ctx.silence = true;
    ctx.input_sync = true;
    ctx.profile_name = Some("CustomProfile".to_string());

    let template = "${title}|${iconTitle}|${id}|${directory}|${hostname}|${username}|${columns}|${rows}|${process}|${status.readonly}|${status.silence}|${status.input-sync}|${profile}";
    let expanded = expand_title_tokens_scoped(template, TitleEditScope::Terminal, &ctx);
    assert_eq!(
        expanded,
        "my-term|my-icon|42|/home/tilix/src|my-host|testuser|120|40|zsh|[RO]|[Silence]|[Sync]|CustomProfile"
    );

    // Verify boolean flags render empty string when false
    let mut ctx_false = TokenContext::new_terminal("term2");
    ctx_false.readonly = false;
    ctx_false.silence = false;
    ctx_false.input_sync = false;
    let res_false = expand_title_tokens_scoped(
        "${status.readonly}${status.silence}${status.input-sync}",
        TitleEditScope::Terminal,
        &ctx_false,
    );
    assert_eq!(res_false, "");
}

/// 3. Evaluation of Session tokens alongside terminal tokens and scope restrictions.
#[test]
fn test_phase11_session_token_catalog() {
    let mut ctx = TokenContext::new_session("active-pane", 3, 2);
    ctx.id = Some(5);
    ctx.profile_name = Some("Default".to_string());

    let template = "Session: ${activeTerminalTitle} [${terminalNumber}/${terminalCount}] (ID:${id}, Prof:${profile})";
    let expanded = expand_title_tokens_scoped(template, TitleEditScope::Session, &ctx);
    assert_eq!(expanded, "Session: active-pane [2/3] (ID:5, Prof:Default)");

    // Window scope tokens like ${sessionCount} should NOT expand in Session scope
    let window_in_session = "${sessionCount} - ${sessionNumber} - ${activeTerminalTitle}";
    let expanded_unscoped = expand_title_tokens_scoped(window_in_session, TitleEditScope::Session, &ctx);
    assert_eq!(expanded_unscoped, "${sessionCount} - ${sessionNumber} - active-pane");
}

/// 4. Evaluation of Window tokens alongside session and terminal tokens.
#[test]
fn test_phase11_window_token_catalog() {
    let mut ctx = TokenContext::new_window("Main Session", 4, 1);
    ctx.title = "active-pane".to_string();
    ctx.active_terminal_title = Some("active-pane".to_string());
    ctx.terminal_count = Some(2);
    ctx.terminal_number = Some(1);

    let template = "${appName}: ${sessionName} (${sessionNumber} of ${sessionCount}) - ${activeTerminalTitle}";
    let expanded = expand_title_tokens_scoped(template, TitleEditScope::Window, &ctx);
    assert_eq!(expanded, "Tilix: Main Session (1 of 4) - active-pane");
}

/// 5. Edge cases: unknown tokens, unclosed syntax, empty contexts, and duplicate tokens.
#[test]
fn test_phase11_token_edge_cases() {
    let ctx = TokenContext::default();

    // Unknown token remains literal
    let unknown = expand_title_tokens("${unknown_token} and ${foo}", &ctx);
    assert_eq!(unknown, "${unknown_token} and ${foo}");

    // Unclosed token remains literal
    let unclosed = expand_title_tokens("prefix ${unclosed and suffix", &ctx);
    assert_eq!(unclosed, "prefix ${unclosed and suffix");

    // Empty format string
    let empty = expand_title_tokens("", &ctx);
    assert_eq!(empty, "");

    // Duplicate tokens
    let dup = expand_title_tokens("${title} - ${title}", &ctx);
    assert_eq!(dup, "Terminal - Terminal");

    // Default fallbacks with empty context
    let fallbacks = expand_title_tokens("${columns}x${rows} on ${appName}", &ctx);
    assert_eq!(fallbacks, "80x24 on Tilix");
}

/// 6. Appearance preferences page displays "Default session name" and "Application title" rows.
#[test]
fn test_phase11_preferences_appearance_title_rows() {
    run_gtk_test(|| {
        let pref_win = TilixPreferencesWindow::new(None::<&gtk::Window>, |_| {});
        let root = pref_win.window();

        let session_label = find_label_by_text(root, "Default session name");
        assert!(session_label.is_some(), "Expected 'Default session name' row in Appearance page");

        let app_label = find_label_by_text(root, "Application title");
        assert!(app_label.is_some(), "Expected 'Application title' row in Appearance page");

        // Verify entries contain default values
        let entries = find_widgets::<gtk::Entry>(root);
        let has_session_entry = entries.iter().any(|e| e.text().as_str() == "${title}");
        let has_app_entry = entries.iter().any(|e| e.text().as_str() == "${appName}: ${sessionName}");
        assert!(has_session_entry, "Expected entry with default session name template");
        assert!(has_app_entry, "Expected entry with default app title template");
    });
}

/// 7. Reusable scoped token menu popover inserts token at cursor position without wiping text.
#[test]
fn test_phase11_token_menu_popover_cursor_insertion() {
    run_gtk_test(|| {
        let entry = gtk::Entry::new();
        entry.set_text("Hello ");
        entry.set_position(6);

        let menu_btn = create_scoped_token_menu_button(&entry, TitleEditScope::Window);
        let popover = menu_btn.popover().expect("MenuButton must have popover");

        let popover_widget: gtk::Widget = popover.upcast();
        let session_btn = find_button_by_label(&popover_widget, "${sessionName}")
            .expect("Expected ${sessionName} button in popover");

        session_btn.emit_clicked();

        assert_eq!(entry.text().as_str(), "Hello ${sessionName}");
        assert_eq!(entry.position(), ("Hello ${sessionName}".chars().count() as i32));
    });
}

/// 8. Live dynamic title synchronization across session view and window header bar.
#[test]
fn test_phase11_dynamic_title_synchronization() {
    run_gtk_test(|| {
        let app = adw::Application::builder()
            .application_id("com.github.tilix_rust.test_phase11_sync")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        let win = TilixWindow::new(&app);
        let tab_view = win.tab_view().clone();
        assert_eq!(tab_view.n_pages(), 1);

        let page = tab_view.nth_page(0);
        assert_eq!(page.title().as_str(), "Terminal");
        assert_eq!(win.title_widget().title().as_str(), "Tilix: Terminal");

        // Update active terminal pane title and synchronize
        if let Some(session_rc) = win.session_view() {
            if let Some(id) = session_rc.borrow().active_pane_id() {
                if let Some(pane) = session_rc.borrow().panes().borrow().get(&id) {
                    pane.set_title("bash");
                }
            }
            session_rc.borrow().notify_title_changed();
        }
        win.update_window_and_tab_titles();

        assert_eq!(page.title().as_str(), "bash");
        assert_eq!(win.title_widget().title().as_str(), "Tilix: bash");
    });
}

/// 9. Regression test: focusing a pane preserves ${directory} in titles without wipeout.
#[test]
fn test_phase11_focus_preserves_directory_in_title() {
    run_gtk_test(|| {
        let mut cfg = AppConfig::load();
        let prev_session_name = cfg.default_session_name.clone();
        let prev_app_title = cfg.app_title.clone();

        cfg.default_session_name = "${title} - ${directory}".to_string();
        cfg.app_title = "${appName}: ${directory}".to_string();
        let _ = cfg.save();

        let app = adw::Application::builder()
            .application_id("com.github.tilix_rust.test_phase11_focus_dir")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        let win = TilixWindow::new(&app);
        let tab_view = win.tab_view().clone();
        assert_eq!(tab_view.n_pages(), 1);

        let page = tab_view.nth_page(0);
        let session_rc = win.session_view().expect("Expected session view");

        // Set pane directory
        let id1 = session_rc.borrow().active_pane_id().expect("Expected active pane");
        let test_dir = std::path::PathBuf::from("/test/project/dir");
        if let Some(pane) = session_rc.borrow().panes().borrow().get(&id1) {
            pane.set_current_directory(Some(test_dir.clone()));
        }

        // Trigger title update
        session_rc.borrow().notify_title_changed();
        win.update_window_and_tab_titles();

        assert_eq!(page.title().as_str(), "Terminal - /test/project/dir");
        assert_eq!(win.title_widget().title().as_str(), "Tilix: /test/project/dir");

        // Now simulate focusing the pane (which previously caused borrow_mut conflict and wiped ${directory})
        session_rc.borrow().set_active_pane(id1);
        win.update_window_and_tab_titles();

        // Must still show directory, not empty!
        assert_eq!(page.title().as_str(), "Terminal - /test/project/dir");
        assert_eq!(win.title_widget().title().as_str(), "Tilix: /test/project/dir");

        // Restore config
        let mut restore_cfg = AppConfig::load();
        restore_cfg.default_session_name = prev_session_name;
        restore_cfg.app_title = prev_app_title;
        let _ = restore_cfg.save();
    });
}

