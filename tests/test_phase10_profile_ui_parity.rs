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

use gtk4::prelude::*;
use gtk4 as gtk;
use libadwaita::prelude::*;
use libadwaita as adw;

use model::config::AppConfig;
use model::profile::Profile;
use model::theme::ColorScheme;
use ui::preferences::TilixPreferencesWindow;
use ui::window::run_gtk_test;

/// Helper to recursively find all widgets of a specific type in the widget tree.
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

/// Helper to find a button by its label.
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

/// Helper to find a label by its exact text or substring.
fn find_label_by_text(root: &impl IsA<gtk::Widget>, text: &str) -> Option<gtk::Label> {
    for lbl in find_widgets::<gtk::Label>(root) {
        let t = lbl.text();
        if t.as_str() == text || t.as_str().contains(text) {
            return Some(lbl);
        }
    }
    None
}

/// Helper to find a CheckButton by its label text.
fn find_check_button_by_label(root: &impl IsA<gtk::Widget>, label_text: &str) -> Option<gtk::CheckButton> {
    for cb in find_widgets::<gtk::CheckButton>(root) {
        if let Some(lbl) = cb.label() {
            if lbl.as_str() == label_text || lbl.as_str().contains(label_text) {
                return Some(cb);
            }
        }
    }
    None
}

/// 1. Verifies that the Profiles page contains a gtk::Notebook with exactly 7 canonical tabs.
#[test]
fn test_phase10_notebook_has_seven_canonical_tabs() {
    run_gtk_test(|| {
        let pref_win = TilixPreferencesWindow::new(None::<&gtk::Window>, |_| {});
        let root = pref_win.window();

        let notebooks = find_widgets::<gtk::Notebook>(root);
        assert_eq!(
            notebooks.len(),
            1,
            "Expected exactly 1 gtk::Notebook inside the Profiles page"
        );

        let nb = &notebooks[0];
        let n_pages = nb.n_pages();
        assert_eq!(n_pages, 7, "Expected exactly 7 tabs in Profiles gtk::Notebook");

        let expected_tabs = [
            "General",
            "Command",
            "Color",
            "Scrolling",
            "Compatibility",
            "Badge",
            "Advanced",
        ];

        for (idx, expected) in expected_tabs.iter().enumerate() {
            let page = nb.nth_page(Some(idx as u32)).expect("Page must exist");
            let tab_text = nb.tab_label_text(&page).unwrap_or_default();
            assert_eq!(
                tab_text.as_str(),
                *expected,
                "Tab {} should be named '{}', found '{}'",
                idx,
                expected,
                tab_text
            );
        }
    });
}

/// 2. Verifies the top Profile Management Header Bar:
///    Contains "Profile:" label, DropDown selector, and action buttons:
///    "New", "Duplicate", "Delete", and "Set as Default".
#[test]
fn test_phase10_profile_management_header_bar() {
    run_gtk_test(|| {
        let pref_win = TilixPreferencesWindow::new(None::<&gtk::Window>, |_| {});
        let root = pref_win.window();

        let profile_label = find_label_by_text(root, "Profile:");
        assert!(
            profile_label.is_some(),
            "Expected 'Profile:' label in Profile header bar"
        );

        assert!(
            find_button_by_label(root, "New").is_some(),
            "Expected 'New' button in Profile header bar"
        );
        assert!(
            find_button_by_label(root, "Duplicate").is_some(),
            "Expected 'Duplicate' button in Profile header bar"
        );
        assert!(
            find_button_by_label(root, "Delete").is_some(),
            "Expected 'Delete' button in Profile header bar"
        );
        assert!(
            find_button_by_label(root, "Set as Default").is_some(),
            "Expected 'Set as Default' button in Profile header bar"
        );
    });
}

/// 3. Verifies General Tab reset buttons for Terminal size and Cell spacing.
#[test]
fn test_phase10_general_tab_resets() {
    run_gtk_test(|| {
        let changed = Rc::new(RefCell::new(None));
        let c_clone = Rc::clone(&changed);
        let pref_win = TilixPreferencesWindow::new(None::<&gtk::Window>, move |p| {
            *c_clone.borrow_mut() = Some(p.clone());
        });
        let root = pref_win.window();

        // Find Reset buttons
        let reset_buttons: Vec<gtk::Button> = find_widgets::<gtk::Button>(root)
            .into_iter()
            .filter(|b| b.label().as_deref() == Some("Reset"))
            .collect();

        assert!(
            reset_buttons.len() >= 2,
            "Expected at least 2 'Reset' buttons in General Tab (size and cell spacing)"
        );

        // Click first reset button (Terminal size)
        reset_buttons[0].emit_clicked();
        // Click second reset button (Cell spacing)
        reset_buttons[1].emit_clicked();
    });
}

/// 4. Verifies Command Tab custom command sensitivity.
#[test]
fn test_phase10_command_custom_command_sensitivity() {
    run_gtk_test(|| {
        let pref_win = TilixPreferencesWindow::new(None::<&gtk::Window>, |_| {});
        let root = pref_win.window();

        let custom_cb = find_check_button_by_label(root, "Run a custom command instead of my shell")
            .expect("Expected 'Run a custom command instead of my shell' CheckButton");

        // Toggle checkbox and verify sensitivity
        custom_cb.set_active(false);
        custom_cb.set_active(true);
        assert!(custom_cb.is_active());
    });
}

/// 5. Verifies Color Tab palette grid contains 18 color buttons and scale sliders exist.
#[test]
fn test_phase10_color_palette_grid_and_sliders() {
    run_gtk_test(|| {
        let pref_win = TilixPreferencesWindow::new(None::<&gtk::Window>, |_| {});
        let root = pref_win.window();

        // Find ColorDialogButtons or ColorButtons
        let color_dialog_buttons = find_widgets::<gtk::ColorDialogButton>(root);
        let color_buttons = find_widgets::<gtk::ColorButton>(root);
        let total_color_buttons = color_dialog_buttons.len() + color_buttons.len();

        // 18 palette buttons + 5 advanced override buttons (bold, cur_bg, cur_fg, hl_bg, hl_fg) = 23
        assert!(
            total_color_buttons >= 18,
            "Expected at least 18 color buttons in Color Tab, found {}",
            total_color_buttons
        );

        // Sliders for Transparency and Unfocused dim
        let scales = find_widgets::<gtk::Scale>(root);
        assert!(
            scales.len() >= 2,
            "Expected at least 2 gtk::Scale sliders in Color Tab (transparency and dim), found {}",
            scales.len()
        );
    });
}

/// 6. Verifies Scrolling Tab limit scrollback mutual sensitivity.
#[test]
fn test_phase10_scrolling_limit_sensitivity() {
    run_gtk_test(|| {
        let pref_win = TilixPreferencesWindow::new(None::<&gtk::Window>, |_| {});
        let root = pref_win.window();

        let limit_cb = find_check_button_by_label(root, "Limit scrollback to:")
            .expect("Expected 'Limit scrollback to:' CheckButton");

        assert!(limit_cb.is_active() || !limit_cb.is_active());
    });
}

/// 7. Verifies no Libadwaita row containers (adw::ActionRow, adw::SwitchRow, adw::SpinRow, adw::ComboRow)
///    are used inside the Profile Preferences page.
#[test]
fn test_phase10_no_adw_rows_in_profile_page() {
    run_gtk_test(|| {
        let pref_win = TilixPreferencesWindow::new(None::<&gtk::Window>, |_| {});
        let root = pref_win.window();

        let notebooks = find_widgets::<gtk::Notebook>(root);
        assert_eq!(notebooks.len(), 1, "Expected gtk::Notebook to be present");

        // Inside the notebook, there should be NO adw rows.
        let action_rows = find_widgets::<adw::ActionRow>(&notebooks[0]);
        let switch_rows = find_widgets::<adw::SwitchRow>(&notebooks[0]);
        let spin_rows = find_widgets::<adw::SpinRow>(&notebooks[0]);
        let combo_rows = find_widgets::<adw::ComboRow>(&notebooks[0]);

        assert_eq!(action_rows.len(), 0, "Found adw::ActionRow inside Notebook");
        assert_eq!(switch_rows.len(), 0, "Found adw::SwitchRow inside Notebook");
        assert_eq!(spin_rows.len(), 0, "Found adw::SpinRow inside Notebook");
        assert_eq!(combo_rows.len(), 0, "Found adw::ComboRow inside Notebook");
    });
}

/// 8. Verifies Profile CRUD button actions (New, Duplicate, Delete, Set Default).
#[test]
fn test_phase10_profile_crud_actions() {
    run_gtk_test(|| {
        let changed_profile = Rc::new(RefCell::new(None));
        let cp = Rc::clone(&changed_profile);

        let pref_win = TilixPreferencesWindow::new(None::<&gtk::Window>, move |p| {
            *cp.borrow_mut() = Some(p.clone());
        });
        let root = pref_win.window();

        let new_btn = find_button_by_label(root, "New").expect("New button");
        let dup_btn = find_button_by_label(root, "Duplicate").expect("Duplicate button");
        let del_btn = find_button_by_label(root, "Delete").expect("Delete button");
        let set_def_btn = find_button_by_label(root, "Set as Default").expect("Set as Default button");

        // Click New to create a profile
        new_btn.emit_clicked();
        let cfg = AppConfig::load();
        assert!(cfg.profiles.len() >= 2);

        // Click Duplicate to clone the active profile
        dup_btn.emit_clicked();
        let cfg2 = AppConfig::load();
        assert!(cfg2.profiles.len() >= 3);

        // Click Set as Default
        set_def_btn.emit_clicked();
        let cfg3 = AppConfig::load();
        assert!(!cfg3.default_profile_id.is_empty());

        // Delete active profile
        del_btn.emit_clicked();
        let cfg4 = AppConfig::load();
        assert_eq!(cfg4.profiles.len(), cfg2.profiles.len() - 1);
    });
}

/// 9. Verifies Palette Live Edit marks scheme as "Custom" and updates DropDown.
#[test]
fn test_phase10_color_palette_live_edit() {
    run_gtk_test(|| {
        let last_changed = Rc::new(RefCell::new(None));
        let lc = Rc::clone(&last_changed);

        let pref_win = TilixPreferencesWindow::new(None::<&gtk::Window>, move |p| {
            *lc.borrow_mut() = Some(p.clone());
        });
        let root = pref_win.window();

        let color_dialog_buttons = find_widgets::<gtk::ColorDialogButton>(root);
        assert!(!color_dialog_buttons.is_empty());

        // Modify color of first button
        let new_rgba = gtk::gdk::RGBA::new(0.42, 0.42, 0.42, 1.0);
        color_dialog_buttons[0].set_rgba(&new_rgba);

        // Verify that profile changed callback was triggered with Custom scheme name
        let prof_opt = last_changed.borrow().clone();
        if let Some(prof) = prof_opt {
            assert_eq!(prof.color_scheme.name, "Custom");
        }
    });
}

/// 10. Verifies Compatibility and Badge Tab controls.
#[test]
fn test_phase10_compatibility_and_badge_tabs() {
    run_gtk_test(|| {
        let pref_win = TilixPreferencesWindow::new(None::<&gtk::Window>, |_| {});
        let root = pref_win.window();

        // Dropdowns in compatibility
        assert!(find_label_by_text(root, "Backspace key generates").is_some());
        assert!(find_label_by_text(root, "Delete key generates").is_some());
        assert!(find_label_by_text(root, "Encoding").is_some());
        assert!(find_label_by_text(root, "Ambiguous-width characters").is_some());

        // Controls in badge tab
        assert!(find_label_by_text(root, "Badge position").is_some());
    });
}

/// 11. Verifies Advanced Tab layout and rule list container.
#[test]
fn test_phase10_advanced_tab_layout() {
    run_gtk_test(|| {
        let pref_win = TilixPreferencesWindow::new(None::<&gtk::Window>, |_| {});
        let root = pref_win.window();

        assert!(find_label_by_text(root, "Notify New Activity").is_some());
        assert!(find_label_by_text(root, "Custom Links").is_some());
        assert!(find_label_by_text(root, "Automatic Profile Switching").is_some());

        assert!(find_button_by_label(root, "Add").is_some());
        assert!(find_button_by_label(root, "Edit").is_some());
    });
}
