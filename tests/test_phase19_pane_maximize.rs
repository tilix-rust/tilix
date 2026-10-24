#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(deprecated)]

use tilix::{model, ui};

use gtk4 as gtk;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use model::keybindings::{ActionCategory, KeybindingsConfig, ACTION_CATALOG};
use model::layout::{Direction, DockPosition, LayoutNode, PaneId, SplitId, SplitOrientation};
use model::session::SessionModel;
use ui::quake::TilixQuakeWindow;
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

fn find_label_in_menu(model: &impl IsA<gio::MenuModel>, target: &str) -> bool {
    let model = model.as_ref();
    for i in 0..model.n_items() {
        if let Some(attr) = model.item_attribute_value(i, "label", None) {
            if let Some(s) = attr.str() {
                if s == target {
                    return true;
                }
            }
        }
        if let Some(section) = model.item_link(i, "section") {
            if find_label_in_menu(&section, target) {
                return true;
            }
        }
        if let Some(submenu) = model.item_link(i, "submenu") {
            if find_label_in_menu(&submenu, target) {
                return true;
            }
        }
    }
    false
}

/// 1. Verifies SessionModel maximize lifecycle (single pane guard, toggle, target switch, unmaximize).
#[test]
fn test_session_model_maximize_lifecycle() {
    let mut session = SessionModel::new(PaneId(1));
    assert!(!session.is_maximized());
    assert_eq!(session.maximized_pane(), None);

    // Single-pane session: maximize is a no-op
    assert!(!session.toggle_maximize(None));
    assert!(!session.is_maximized());
    assert_eq!(session.maximized_pane(), None);

    assert!(!session.toggle_maximize(Some(PaneId(1))));
    assert!(!session.is_maximized());
    assert_eq!(session.maximized_pane(), None);

    // Split active horizontal -> 2 panes (PaneId(1) and PaneId(2))
    let p2 = session.split_active(SplitOrientation::Horizontal).unwrap();
    assert_eq!(session.active_pane, Some(p2));

    // Maximize active (p2)
    assert!(session.toggle_maximize(None));
    assert!(session.is_maximized());
    assert_eq!(session.maximized_pane(), Some(p2));

    // Toggle again -> unmaximize
    assert!(!session.toggle_maximize(None));
    assert!(!session.is_maximized());
    assert_eq!(session.maximized_pane(), None);

    // Maximize specific pane (p1)
    assert!(session.toggle_maximize(Some(PaneId(1))));
    assert!(session.is_maximized());
    assert_eq!(session.maximized_pane(), Some(PaneId(1)));
    assert_eq!(session.active_pane, Some(PaneId(1)));

    // Switch maximize directly to p2
    assert!(session.toggle_maximize(Some(p2)));
    assert!(session.is_maximized());
    assert_eq!(session.maximized_pane(), Some(p2));
    assert_eq!(session.active_pane, Some(p2));

    // Explicit unmaximize
    assert!(session.unmaximize());
    assert!(!session.is_maximized());
    assert_eq!(session.maximized_pane(), None);
    assert!(!session.unmaximize());
}

/// 2. Verifies that splitting, docking, or adopting automatically unmaximizes the session.
#[test]
fn test_session_model_split_auto_unmaximize() {
    let mut session = SessionModel::new(PaneId(1));
    let p2 = session.split_active(SplitOrientation::Horizontal).unwrap();

    // Maximize p1
    assert!(session.toggle_maximize(Some(PaneId(1))));
    assert!(session.is_maximized());
    assert_eq!(session.maximized_pane(), Some(PaneId(1)));

    // Splitting p1 automatically resets maximized state
    let p3 = session.split_pane(PaneId(1), SplitOrientation::Vertical).unwrap();
    assert!(!session.is_maximized());
    assert_eq!(session.maximized_pane(), None);
    assert_eq!(session.active_pane, Some(p3));

    // Maximize p2, then dock p3 into p2 -> resets maximized state
    assert!(session.toggle_maximize(Some(p2)));
    assert!(session.is_maximized());
    session.dock_pane(p3, p2, DockPosition::Bottom).unwrap();
    assert!(!session.is_maximized());
    assert_eq!(session.maximized_pane(), None);

    // Maximize p1, then adopt a pane -> resets maximized state
    assert!(session.toggle_maximize(Some(PaneId(1))));
    assert!(session.is_maximized());
    session.adopt_pane(PaneId(99), PaneId(1), DockPosition::Right).unwrap();
    assert!(!session.is_maximized());
    assert_eq!(session.maximized_pane(), None);
}

/// 3. Verifies that closing a maximized pane falls back cleanly and remaining <= 1 auto-unmaximizes.
#[test]
fn test_session_model_close_maximized_fallback() {
    let mut session = SessionModel::new(PaneId(1));
    let p2 = session.split_active(SplitOrientation::Horizontal).unwrap();
    let p3 = session.split_active(SplitOrientation::Vertical).unwrap();

    // Maximize p3 and close it
    assert!(session.toggle_maximize(Some(p3)));
    assert!(session.is_maximized());
    let fallback = session.close_pane(p3).unwrap();
    assert!(!session.is_maximized());
    assert_eq!(session.maximized_pane(), None);
    assert_eq!(fallback, Some(p2));

    // Maximize p1 in 2-pane session
    assert!(session.toggle_maximize(Some(PaneId(1))));
    assert!(session.is_maximized());
    assert_eq!(session.maximized_pane(), Some(PaneId(1)));

    // Close peer pane p2 -> only 1 pane remaining -> auto-unmaximize
    session.close_pane(p2).unwrap();
    assert!(!session.is_maximized());
    assert_eq!(session.maximized_pane(), None);
}

/// 4. Verifies action catalog integrity for win.terminal-maximize and default shortcuts.
#[test]
fn test_action_catalog_terminal_maximize_integrity() {
    assert_eq!(ACTION_CATALOG.len(), 34);

    let max_action = ACTION_CATALOG
        .iter()
        .find(|d| d.id == "win.terminal-maximize")
        .expect("win.terminal-maximize must exist in ACTION_CATALOG");

    assert_eq!(max_action.title, "Maximize Terminal");
    assert_eq!(max_action.description, "Toggle maximize active terminal pane");
    assert_eq!(max_action.category, ActionCategory::SplitsAndLayout);
    assert_eq!(max_action.default_accels, &["<Primary><Shift>x", "<Alt>m"]);

    let config = KeybindingsConfig::default();
    assert_eq!(
        config.get_effective_accel("win.terminal-maximize"),
        Some("<Primary><Shift>x".to_string())
    );
    assert_eq!(
        config.get_all_effective_accels("win.terminal-maximize"),
        vec!["<Primary><Shift>x".to_string(), "<Alt>m".to_string()]
    );
}

/// 5. Verifies SessionView widget projection bypass, direct mounting, and split ratio retention.
#[test]
fn test_session_view_maximize_projection_and_ratio_retention() {
    run_gtk_test(|| {
        let session = SessionView::new();
        assert_eq!(session.pane_count(), 1);

        // Single pane cannot be maximized
        assert!(!session.toggle_maximize_active());
        assert!(!session.is_maximized());

        // Split horizontally into 2 panes
        session.split_active(SplitOrientation::Horizontal);
        assert_eq!(session.pane_count(), 2);

        // Set a custom split ratio (0.75) on the root node
        session.model_mut().layout.set_split_ratio(SplitId(1), 0.75);
        let root_ratio = match session.model().layout.root() {
            Some(LayoutNode::Split { ratio, .. }) => *ratio,
            _ => 0.0,
        };
        assert!((root_ratio - 0.75).abs() < 1e-6);

        // Normal layout has GtkPaned as root container child
        let root_child = session.container().first_child().expect("Root child must exist");
        assert!(root_child.downcast_ref::<gtk::Paned>().is_some(), "Root child in split layout must be a Paned");

        // Maximize active pane
        assert!(session.toggle_maximize_active());
        assert!(session.is_maximized());

        // Under maximized mode, container's child is directly a TerminalPane (not GtkPaned)
        let max_child = session.container().first_child().expect("Maximized child must exist");
        assert!(max_child.downcast_ref::<gtk::Paned>().is_none(), "Maximized child must NOT be a Paned");

        // Capturing layout ratios while maximized MUST be guarded and NOT overwrite or corrupt split ratio
        session.capture_layout_ratios();
        let ratio_during_max = match session.model().layout.root() {
            Some(LayoutNode::Split { ratio, .. }) => *ratio,
            _ => 0.0,
        };
        assert!((ratio_during_max - 0.75).abs() < 1e-6, "Ratio must not drift while maximized");

        // Unmaximize restores Paned hierarchy and exact split ratio
        assert!(session.unmaximize());
        assert!(!session.is_maximized());

        let restored_child = session.container().first_child().expect("Restored child must exist");
        assert!(restored_child.downcast_ref::<gtk::Paned>().is_some(), "Restored child must be a Paned");

        let ratio_restored = match session.model().layout.root() {
            Some(LayoutNode::Split { ratio, .. }) => *ratio,
            _ => 0.0,
        };
        assert!((ratio_restored - 0.75).abs() < 1e-6, "Ratio must remain pristine after restore");
    });
}

/// 6. Verifies that right-click context menus include win.terminal-maximize and dynamic labels.
#[test]
fn test_context_menu_maximize_item_present() {
    run_gtk_test(|| {
        let menu_without_link = TerminalPane::build_context_menu(false);
        let menu_with_link = TerminalPane::build_context_menu(true);

        assert!(
            find_action_in_menu(&menu_without_link, "win.terminal-maximize"),
            "Context menu without link must contain win.terminal-maximize"
        );
        assert!(
            find_action_in_menu(&menu_with_link, "win.terminal-maximize"),
            "Context menu with link must contain win.terminal-maximize"
        );

        // Dynamic label testing: Maximize Terminal when unmaximized vs Restore Terminal when maximized
        let unmax_menu = TerminalPane::build_context_menu_with_state(false, false);
        assert!(
            find_label_in_menu(&unmax_menu, "Maximize Terminal"),
            "Context menu when unmaximized must show 'Maximize Terminal'"
        );
        assert!(
            !find_label_in_menu(&unmax_menu, "Restore Terminal"),
            "Context menu when unmaximized must NOT show 'Restore Terminal'"
        );

        let max_menu = TerminalPane::build_context_menu_with_state(false, true);
        assert!(
            find_label_in_menu(&max_menu, "Restore Terminal"),
            "Context menu when maximized must show 'Restore Terminal'"
        );
        assert!(
            !find_label_in_menu(&max_menu, "Maximize Terminal"),
            "Context menu when maximized must NOT show 'Maximize Terminal'"
        );
    });
}

/// 7. Verifies that TilixWindow registers action 'terminal-maximize'.
#[test]
fn test_window_terminal_maximize_action_registered() {
    run_gtk_test(|| {
        let app = adw::Application::builder()
            .application_id("com.gexperts.Tilix.TestPhase19")
            .build();

        let window = TilixWindow::new_empty(&app);
        let win_widget = window.window();

        assert!(
            win_widget.lookup_action("terminal-maximize").is_some(),
            "Action 'terminal-maximize' must be registered on TilixWindow"
        );
    });
}

/// 8. Verifies that TilixQuakeWindow registers action 'terminal-maximize'.
#[test]
fn test_quake_window_terminal_maximize_action_registered() {
    run_gtk_test(|| {
        let app = adw::Application::builder()
            .application_id("com.gexperts.Tilix.TestPhase19Quake")
            .build();

        let quake = TilixQuakeWindow::new(&app);
        assert!(
            quake.window().lookup_action("terminal-maximize").is_some(),
            "Action 'terminal-maximize' must be registered on TilixQuakeWindow"
        );
    });
}

/// 9. Verifies that focus_adjacent is blocked when session is maximized.
#[test]
fn test_maximize_blocks_focus_adjacent() {
    run_gtk_test(|| {
        let session = SessionView::new();
        let p1 = session.active_pane_id().unwrap();
        session.split_active(SplitOrientation::Horizontal);
        let p2 = session.active_pane_id().unwrap();
        assert_ne!(p1, p2);
        assert_eq!(session.active_pane_id(), Some(p2));

        // Switch to p1 and maximize
        session.set_active_pane(p1);
        assert_eq!(session.active_pane_id(), Some(p1));
        assert!(session.toggle_maximize(Some(p1)));
        assert!(session.is_maximized());

        // Attempting to move focus adjacent right must be blocked
        session.focus_adjacent(Direction::Right);
        assert_eq!(
            session.active_pane_id(),
            Some(p1),
            "Focus must NOT move to hidden pane while maximized"
        );

        // Unmaximize restores normal focus navigation
        assert!(session.unmaximize());
        assert!(!session.is_maximized());
        session.focus_adjacent(Direction::Right);
        assert_eq!(session.active_pane_id(), Some(p2));
    });
}
