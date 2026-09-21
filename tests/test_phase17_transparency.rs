#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(deprecated)]

use tilix::{app, model, pty, ui};

use gtk4 as gtk;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use model::profile::Profile;
use model::SplitOrientation;
use ui::preferences::TilixPreferencesWindow;
use ui::quake::TilixQuakeWindow;
use ui::session_view::SessionView;
use ui::terminal_pane::TerminalPane;
use ui::window::{
    apply_profile_to_all_sessions, register_transparency_updater, setup_css, TilixWindow,
};

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

/// 1. setup_css loads and parses transparency and container passthrough rules without errors.
#[test]
fn test_phase17_setup_css_loads_transparency_rules() {
    run_gtk_test(|| {
        setup_css();
    });
}

/// 2. TerminalPane::is_transparent correctly detects background_transparency_percent > 0.
#[test]
fn test_phase17_terminal_pane_is_transparent() {
    run_gtk_test(|| {
        let pane = TerminalPane::new(model::PaneId(1), None);
        let mut opaque_prof = Profile::default();
        opaque_prof.background_transparency_percent = 0;
        pane.apply_profile(&opaque_prof);
        assert!(!pane.is_transparent(), "Pane with 0% transparency should not be transparent");

        let mut trans_prof = Profile::default();
        trans_prof.background_transparency_percent = 30;
        pane.apply_profile(&trans_prof);
        assert!(pane.is_transparent(), "Pane with 30% transparency should be transparent");
    });
}

/// 3. SessionView::has_transparent_pane detects transparent panes in multi-pane split topologies.
#[test]
fn test_phase17_session_view_has_transparent_pane() {
    run_gtk_test(|| {
        let session = SessionView::new();
        let mut opaque_prof = Profile::default();
        opaque_prof.background_transparency_percent = 0;

        let mut trans_prof = Profile::default();
        trans_prof.background_transparency_percent = 50;

        session.apply_profile(&opaque_prof);
        assert!(!session.has_transparent_pane(), "Session with only opaque pane must return false");

        session.split_active(SplitOrientation::Horizontal);
        assert_eq!(session.pane_count(), 2);

        let active_id = session.active_pane_id().unwrap();
        let panes = session.panes();
        if let Some(active_pane) = panes.borrow().get(&active_id) {
            active_pane.apply_profile(&trans_prof);
        }
        assert!(session.has_transparent_pane(), "Session with a transparent split pane must return true");

        // Revert all panes to opaque
        session.apply_profile(&opaque_prof);
        assert!(!session.has_transparent_pane(), "Session with all opaque panes must return false");

        session.close();
    });
}

/// 4. TilixWindow applies .transparent-window initially when initialized with transparent profile.
#[test]
fn test_phase17_tilix_window_initialization_with_transparency() {
    run_gtk_test(|| {
        let app = adw::Application::builder()
            .application_id("com.github.tilix_rust.test_phase17_init")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();

        let mut trans_prof = Profile::default();
        trans_prof.background_transparency_percent = 40;
        let trans_win = TilixWindow::new_empty_with_profile(&app, &trans_prof);
        assert!(
            trans_win.window().has_css_class("transparent-window"),
            "Window initialized with transparent profile must have transparent-window class"
        );

        let mut opaque_prof = Profile::default();
        opaque_prof.background_transparency_percent = 0;
        let opaque_win = TilixWindow::new_empty_with_profile(&app, &opaque_prof);
        assert!(
            !opaque_win.window().has_css_class("transparent-window"),
            "Window initialized with opaque profile must NOT have transparent-window class"
        );
    });
}

/// 5. Reactive projection toggles .transparent-window dynamically upon global profile broadcasts.
#[test]
fn test_phase17_reactive_projection_profile_broadcast() {
    run_gtk_test(|| {
        let app = adw::Application::builder()
            .application_id("com.github.tilix_rust.test_phase17_broadcast")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();

        let mut opaque_prof = Profile::default();
        opaque_prof.background_transparency_percent = 0;
        let tilix_win = TilixWindow::new_empty_with_profile(&app, &opaque_prof);
        tilix_win.create_tab();

        assert!(!tilix_win.window().has_css_class("transparent-window"));

        let mut trans_prof = Profile::default();
        trans_prof.background_transparency_percent = 45;
        apply_profile_to_all_sessions(&trans_prof);

        assert!(
            tilix_win.window().has_css_class("transparent-window"),
            "Window should dynamically receive .transparent-window when broadcast applied transparent profile"
        );

        apply_profile_to_all_sessions(&opaque_prof);
        assert!(
            !tilix_win.window().has_css_class("transparent-window"),
            "Window should dynamically lose .transparent-window when broadcast applied opaque profile"
        );
    });
}

/// 6. TilixQuakeWindow dynamically gains and loses .transparent-window during profile updates.
#[test]
fn test_phase17_quake_window_transparency() {
    run_gtk_test(|| {
        let app = adw::Application::builder()
            .application_id("com.github.tilix_rust.test_phase17_quake")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();

        let quake = TilixQuakeWindow::new(&app);

        let mut opaque_prof = Profile::default();
        opaque_prof.background_transparency_percent = 0;
        apply_profile_to_all_sessions(&opaque_prof);
        assert!(
            !quake.window().has_css_class("transparent-window"),
            "Quake window should not have transparent-window with opaque profile"
        );

        let mut trans_prof = Profile::default();
        trans_prof.background_transparency_percent = 25;
        apply_profile_to_all_sessions(&trans_prof);
        assert!(
            quake.window().has_css_class("transparent-window"),
            "Quake window should have transparent-window with transparent profile broadcast"
        );

        apply_profile_to_all_sessions(&opaque_prof);
        assert!(
            !quake.window().has_css_class("transparent-window"),
            "Quake window should drop transparent-window when reverting to opaque profile"
        );
    });
}

/// 7. Multi-tab transparency switching toggles .transparent-window based on the active tab's transparency.
#[test]
fn test_phase17_multi_tab_transparency_switching() {
    run_gtk_test(|| {
        let app = adw::Application::builder()
            .application_id("com.github.tilix_rust.test_phase17_tabs")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();

        let mut opaque_prof = Profile::default();
        opaque_prof.background_transparency_percent = 0;

        let tilix_win = TilixWindow::new_empty_with_profile(&app, &opaque_prof);
        let (tab1, sess1) = tilix_win.create_tab();
        sess1.borrow().apply_profile(&opaque_prof);
        tilix_win.update_transparency();
        assert!(!tilix_win.window().has_css_class("transparent-window"));

        let mut trans_prof = Profile::default();
        trans_prof.background_transparency_percent = 50;

        let (tab2, sess2) = tilix_win.create_tab();
        sess2.borrow().apply_profile(&trans_prof);
        tilix_win.tab_view().set_selected_page(&tab2);
        tilix_win.update_transparency();

        assert!(
            tilix_win.window().has_css_class("transparent-window"),
            "Window should have .transparent-window when Tab 2 (transparent) is selected"
        );

        // Switch to Tab 1 (opaque)
        tilix_win.tab_view().set_selected_page(&tab1);
        tilix_win.update_transparency();
        assert!(
            !tilix_win.window().has_css_class("transparent-window"),
            "Window should lose .transparent-window when Tab 1 (opaque) is selected"
        );

        // Switch back to Tab 2 (transparent)
        tilix_win.tab_view().set_selected_page(&tab2);
        tilix_win.update_transparency();
        assert!(
            tilix_win.window().has_css_class("transparent-window"),
            "Window should regain .transparent-window when Tab 2 is re-selected"
        );
    });
}

/// 8. TilixPreferencesWindow CSS isolation: never inherits .transparent-window class.
#[test]
fn test_phase17_preferences_window_css_isolation() {
    run_gtk_test(|| {
        let pref_win = TilixPreferencesWindow::new(None::<&gtk::Window>, |_| {});
        assert!(
            !pref_win.window().has_css_class("transparent-window"),
            "TilixPreferencesWindow must never have .transparent-window class initially"
        );

        let mut trans_prof = Profile::default();
        trans_prof.background_transparency_percent = 60;
        apply_profile_to_all_sessions(&trans_prof);

        assert!(
            !pref_win.window().has_css_class("transparent-window"),
            "TilixPreferencesWindow must remain opaque even after transparency profile broadcast"
        );
    });
}
