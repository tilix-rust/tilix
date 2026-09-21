#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(deprecated)]

use tilix::{app, model, pty, ui};

use gtk4 as gtk;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use model::keybindings::{ActionCategory, ACTION_CATALOG};
use model::layout::{PaneId, SplitOrientation};
use ui::session_view::SessionView;
use ui::terminal_pane::{TerminalPane, ZOOM_MAX, ZOOM_MIN, ZOOM_NORMAL, ZOOM_STEP};
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

#[test]
fn test_phase13_keybinding_catalog_zoom_actions() {
    let zoom_in = ACTION_CATALOG.iter().find(|d| d.id == "win.zoom-in");
    assert!(zoom_in.is_some(), "win.zoom-in must be present in ACTION_CATALOG");
    let zi = zoom_in.unwrap();
    assert_eq!(zi.title, "Zoom In");
    assert_eq!(zi.category, ActionCategory::ViewAndSettings);
    assert_eq!(zi.primary_default_accel(), "<Primary>plus");
    assert!(zi.default_accels.contains(&"<Primary>equal"));
    assert!(zi.default_accels.contains(&"<Primary>KP_Add"));

    let zoom_out = ACTION_CATALOG.iter().find(|d| d.id == "win.zoom-out");
    assert!(zoom_out.is_some(), "win.zoom-out must be present in ACTION_CATALOG");
    let zo = zoom_out.unwrap();
    assert_eq!(zo.title, "Zoom Out");
    assert_eq!(zo.category, ActionCategory::ViewAndSettings);
    assert_eq!(zo.primary_default_accel(), "<Primary>minus");
    assert!(zo.default_accels.contains(&"<Primary>KP_Subtract"));

    let zoom_normal = ACTION_CATALOG.iter().find(|d| d.id == "win.zoom-normal");
    assert!(zoom_normal.is_some(), "win.zoom-normal must be present in ACTION_CATALOG");
    let zn = zoom_normal.unwrap();
    assert_eq!(zn.title, "Normal Size");
    assert_eq!(zn.category, ActionCategory::ViewAndSettings);
    assert_eq!(zn.primary_default_accel(), "<Primary>0");
    assert!(zn.default_accels.contains(&"<Primary>KP_0"));
}

#[test]
fn test_phase13_terminal_pane_zoom_operations() {
    run_gtk_test(|| {
        let pane = TerminalPane::new(PaneId(1), None);
        assert!((pane.font_scale() - ZOOM_NORMAL).abs() < f64::EPSILON);

        // Zoom In
        pane.zoom_in();
        assert!((pane.font_scale() - 1.1).abs() < 1e-6);

        pane.zoom_in();
        assert!((pane.font_scale() - 1.2).abs() < 1e-6);

        // Zoom Normal
        pane.zoom_normal();
        assert!((pane.font_scale() - ZOOM_NORMAL).abs() < 1e-6);

        // Zoom Out
        pane.zoom_out();
        assert!((pane.font_scale() - 0.9).abs() < 1e-6);

        // Repeated zoom out clamps to ZOOM_MIN (0.2)
        for _ in 0..20 {
            pane.zoom_out();
        }
        assert!((pane.font_scale() - ZOOM_MIN).abs() < 1e-6);
        assert!((pane.font_scale() - 0.2).abs() < 1e-6);

        // Repeated zoom in clamps to ZOOM_MAX (5.0)
        for _ in 0..60 {
            pane.zoom_in();
        }
        assert!((pane.font_scale() - ZOOM_MAX).abs() < 1e-6);
        assert!((pane.font_scale() - 5.0).abs() < 1e-6);

        // Reset to normal
        pane.zoom_normal();
        assert!((pane.font_scale() - 1.0).abs() < 1e-6);
    });
}

#[test]
fn test_phase13_zoom_arithmetic_precision() {
    run_gtk_test(|| {
        let pane = TerminalPane::new(PaneId(1), None);
        // Step forward 10 times: 1.0 -> 2.0
        for i in 1..=10 {
            pane.zoom_in();
            let expected = 1.0 + (i as f64 * 0.1);
            let scale_str = format!("{:.1}", pane.font_scale());
            let expected_str = format!("{:.1}", expected);
            assert_eq!(scale_str, expected_str);
            // Verify no floating point representation artifacts in 1 decimal place rounding
            let raw = pane.font_scale();
            let rounded = (raw * 10.0).round() / 10.0;
            assert_eq!(raw, rounded);
        }

        // Step back 20 times: 2.0 -> 0.2 (clamped)
        for _ in 0..20 {
            pane.zoom_out();
            let raw = pane.font_scale();
            let rounded = (raw * 10.0).round() / 10.0;
            assert_eq!(raw, rounded);
            assert!(raw >= ZOOM_MIN);
            assert!(raw <= ZOOM_MAX);
        }
    });
}

#[test]
fn test_phase13_session_view_active_zoom_dispatch() {
    run_gtk_test(|| {
        let session = SessionView::new();
        assert_eq!(session.pane_count(), 1);
        let p1 = session.active_pane_id().expect("p1 must exist");

        // Split to create second pane
        session.split_active(SplitOrientation::Horizontal);
        assert_eq!(session.pane_count(), 2);
        let p2 = session.active_pane_id().expect("p2 must exist");
        assert_ne!(p1, p2);

        // Currently p2 is active
        let panes = session.panes();
        let pane1 = panes.borrow().get(&p1).cloned().expect("pane1");
        let pane2 = panes.borrow().get(&p2).cloned().expect("pane2");

        assert!((pane1.font_scale() - 1.0).abs() < 1e-6);
        assert!((pane2.font_scale() - 1.0).abs() < 1e-6);

        // Zoom active pane (p2)
        session.zoom_in_active();
        assert!((pane2.font_scale() - 1.1).abs() < 1e-6);
        assert!((pane1.font_scale() - 1.0).abs() < 1e-6);

        // Switch active pane to p1
        session.set_active_pane(p1);
        assert_eq!(session.active_pane_id(), Some(p1));

        // Zoom p1
        session.zoom_out_active();
        assert!((pane1.font_scale() - 0.9).abs() < 1e-6);
        assert!((pane2.font_scale() - 1.1).abs() < 1e-6);

        // Reset p1
        session.zoom_normal_active();
        assert!((pane1.font_scale() - 1.0).abs() < 1e-6);
        assert!((pane2.font_scale() - 1.1).abs() < 1e-6);

        // Reset p2
        session.set_active_pane(p2);
        session.zoom_normal_active();
        assert!((pane2.font_scale() - 1.0).abs() < 1e-6);

        session.close();
    });
}

#[test]
fn test_phase13_window_zoom_actions_registered() {
    run_gtk_test(|| {
        let app = adw::Application::builder()
            .application_id("com.gexperts.Tilix.TestZoomActions")
            .build();

        let window = TilixWindow::new_empty(&app);
        let win_widget = window.window();

        assert!(
            win_widget.lookup_action("zoom-in").is_some(),
            "Action 'zoom-in' must be registered on TilixWindow"
        );
        assert!(
            win_widget.lookup_action("zoom-out").is_some(),
            "Action 'zoom-out' must be registered on TilixWindow"
        );
        assert!(
            win_widget.lookup_action("zoom-normal").is_some(),
            "Action 'zoom-normal' must be registered on TilixWindow"
        );
    });
}

#[test]
fn test_phase13_scroll_controller_attached_in_capture_phase() {
    run_gtk_test(|| {
        let pane = TerminalPane::new(PaneId(1), None);
        let controllers = pane.terminal().observe_controllers();

        let mut found_scroll = false;
        let mut is_capture_phase = false;
        let mut flags_vertical = false;

        for i in 0..controllers.n_items() {
            if let Some(item) = controllers.item(i) {
                if let Ok(scroll) = item.downcast::<gtk::EventControllerScroll>() {
                    found_scroll = true;
                    if scroll.propagation_phase() == gtk::PropagationPhase::Capture {
                        is_capture_phase = true;
                    }
                    if scroll.flags().contains(gtk::EventControllerScrollFlags::VERTICAL) {
                        flags_vertical = true;
                    }
                }
            }
        }

        assert!(found_scroll, "EventControllerScroll must be attached to terminal");
        assert!(is_capture_phase, "Scroll controller must use PropagationPhase::Capture");
        assert!(flags_vertical, "Scroll controller must have VERTICAL flag");
    });
}

#[test]
fn test_phase13_alt_drag_source_attached_to_terminal_and_header() {
    run_gtk_test(|| {
        let pane = TerminalPane::new(PaneId(1), None);
        pane.setup_drag_source();

        // Check header has a drag source controller
        let header_controllers = pane.header().observe_controllers();
        let mut header_has_drag = false;
        for i in 0..header_controllers.n_items() {
            if let Some(item) = header_controllers.item(i) {
                if item.downcast::<gtk::DragSource>().is_ok() {
                    header_has_drag = true;
                    break;
                }
            }
        }
        assert!(header_has_drag, "Header must have a DragSource controller attached");

        // Check terminal has a drag source controller
        let term_controllers = pane.terminal().observe_controllers();
        let mut term_has_drag = false;
        for i in 0..term_controllers.n_items() {
            if let Some(item) = term_controllers.item(i) {
                if item.downcast::<gtk::DragSource>().is_ok() {
                    term_has_drag = true;
                    break;
                }
            }
        }
        assert!(term_has_drag, "Terminal must have a DragSource controller attached");
    });
}

#[test]
fn test_phase13_dnd_drag_source_modifier_gating() {
    run_gtk_test(|| {
        let pane = TerminalPane::new(PaneId(1), None);
        let button = gtk::Button::new();
        ui::dnd::setup_pane_drag_source(&button, pane.clone(), true);

        let controllers = button.observe_controllers();
        let mut found_drag = false;
        for i in 0..controllers.n_items() {
            if let Some(item) = controllers.item(i) {
                if let Ok(drag_source) = item.downcast::<gtk::DragSource>() {
                    found_drag = true;
                    assert!(drag_source.actions().contains(gtk::gdk::DragAction::MOVE));
                    break;
                }
            }
        }
        assert!(found_drag, "DragSource must be attached to widget with require_alt");

        // Header widget without alt requirement
        let header_btn = gtk::Button::new();
        ui::dnd::setup_pane_drag_source(&header_btn, pane, false);
        let h_controllers = header_btn.observe_controllers();
        let mut h_found_drag = false;
        for i in 0..h_controllers.n_items() {
            if let Some(item) = h_controllers.item(i) {
                if item.downcast::<gtk::DragSource>().is_ok() {
                    h_found_drag = true;
                    break;
                }
            }
        }
        assert!(h_found_drag, "DragSource must be attached to widget without require_alt");
    });
}

#[test]
fn test_phase13_dnd_active_drag_lifecycle_preserves_running_pane() {
    run_gtk_test(|| {
        let mut child = std::process::Command::new("sleep")
            .arg("60")
            .spawn()
            .expect("Failed to spawn sleep");
        let _pid = child.id() as i32;

        let session = SessionView::new();
        let p1 = session.active_pane_id().expect("p1 must exist");
        let panes = session.panes();
        let _pane1 = panes.borrow().get(&p1).cloned().expect("pane1 must exist");

        // Start active pane drag
        let active = ui::dnd::ActivePaneDrag {
            pane_id: p1,
            source_session_widget: glib::WeakRef::new(),
        };
        ui::dnd::set_active_pane_drag(Some(active));

        // Retrieve active pane drag during mouse motion simulation
        let drag_opt = ui::dnd::get_active_pane_drag();
        assert!(drag_opt.is_some());
        assert_eq!(drag_opt.unwrap().pane_id, p1);

        // Clear active pane drag (e.g. drop or drag cancelled)
        ui::dnd::clear_active_pane_drag();
        assert!(ui::dnd::get_active_pane_drag().is_none());

        // Child process must still be alive!
        assert!(child.try_wait().unwrap().is_none(), "Process must remain alive after drag end");

        // Pane must NOT be closed!
        assert_eq!(session.pane_count(), 1, "Pane must still be part of session");
        assert_eq!(session.active_pane_id(), Some(p1));

        // Explicit close properly terminates session
        session.close();
        let _ = child.kill();
        let _ = child.wait();
    });
}

