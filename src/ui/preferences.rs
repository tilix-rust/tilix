use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;

use crate::model::config::AppConfig;
use crate::model::profile::{CursorBlinkPreference, CursorShapePreference, Profile};
use crate::model::theme::ColorScheme;

#[allow(deprecated)]
pub struct TilixPreferencesWindow {
    window: adw::PreferencesWindow,
}

#[allow(deprecated)]
impl TilixPreferencesWindow {
    pub fn new<F: Fn(&Profile) + 'static>(parent: &impl IsA<gtk::Window>, on_profile_changed: F) -> Self {
        let window = adw::PreferencesWindow::new();
        window.set_transient_for(Some(parent));
        window.set_modal(true);
        window.set_title(Some("Preferences"));

        let current_config = Rc::new(RefCell::new(AppConfig::load()));
        let on_change = Rc::new(on_profile_changed);

        let page = adw::PreferencesPage::new();
        page.set_title("Appearance");
        page.set_icon_name(Some("preferences-desktop-appearance-symbolic"));

        let group = adw::PreferencesGroup::new();
        group.set_title("Terminal Appearance");
        page.add(&group);

        // Color Scheme
        let color_names = ["Tilix Dark", "Tilix Light", "Solarized Dark", "Monokai"];
        let color_model = gtk::StringList::new(&color_names);
        let color_row = adw::ComboRow::new();
        color_row.set_title("Color Scheme");
        color_row.set_model(Some(&color_model));

        let current_color_index = match current_config.borrow().default_profile.color_scheme.name.as_str() {
            "Tilix Light" => 1,
            "Solarized Dark" => 2,
            "Monokai" => 3,
            _ => 0,
        };
        color_row.set_selected(current_color_index);

        // Cursor Shape
        let shape_names = ["Block", "I-Beam", "Underline"];
        let shape_model = gtk::StringList::new(&shape_names);
        let shape_row = adw::ComboRow::new();
        shape_row.set_title("Cursor Shape");
        shape_row.set_model(Some(&shape_model));

        let current_shape_index = match current_config.borrow().default_profile.cursor_shape {
            CursorShapePreference::Block => 0,
            CursorShapePreference::IBeam => 1,
            CursorShapePreference::Underline => 2,
        };
        shape_row.set_selected(current_shape_index);

        // Cursor Blink
        let blink_names = ["System", "On", "Off"];
        let blink_model = gtk::StringList::new(&blink_names);
        let blink_row = adw::ComboRow::new();
        blink_row.set_title("Cursor Blink");
        blink_row.set_model(Some(&blink_model));

        let current_blink_index = match current_config.borrow().default_profile.cursor_blink {
            CursorBlinkPreference::System => 0,
            CursorBlinkPreference::On => 1,
            CursorBlinkPreference::Off => 2,
        };
        blink_row.set_selected(current_blink_index);

        // Font
        let font_row = adw::ActionRow::new();
        font_row.set_title("Font");
        let font_entry = gtk::Entry::new();
        let current_font = current_config
            .borrow()
            .default_profile
            .font
            .as_deref()
            .unwrap_or("Monospace 11")
            .to_string();
        font_entry.set_text(&current_font);
        font_entry.set_valign(gtk::Align::Center);
        font_row.add_suffix(&font_entry);

        group.add(&color_row);
        group.add(&shape_row);
        group.add(&blink_row);
        group.add(&font_row);

        window.add(&page);

        // Helper to apply and save changes
        let sync_and_save = {
            let config_rc = Rc::clone(&current_config);
            let on_change_cb = Rc::clone(&on_change);
            let c_row = color_row.clone();
            let s_row = shape_row.clone();
            let b_row = blink_row.clone();
            let f_entry = font_entry.clone();

            Rc::new(move || {
                let color_scheme = match c_row.selected() {
                    1 => ColorScheme::tilix_light(),
                    2 => ColorScheme::solarized_dark(),
                    3 => ColorScheme::monokai(),
                    _ => ColorScheme::tilix_dark(),
                };

                let cursor_shape = match s_row.selected() {
                    1 => CursorShapePreference::IBeam,
                    2 => CursorShapePreference::Underline,
                    _ => CursorShapePreference::Block,
                };

                let cursor_blink = match b_row.selected() {
                    1 => CursorBlinkPreference::On,
                    2 => CursorBlinkPreference::Off,
                    _ => CursorBlinkPreference::System,
                };

                let font_text = f_entry.text().to_string();
                let font = if font_text.trim().is_empty() {
                    Some("Monospace 11".to_string())
                } else {
                    Some(font_text)
                };

                let mut cfg = config_rc.borrow_mut();
                cfg.default_profile.color_scheme = color_scheme;
                cfg.default_profile.cursor_shape = cursor_shape;
                cfg.default_profile.cursor_blink = cursor_blink;
                cfg.default_profile.font = font;

                let _ = cfg.save();
                on_change_cb(&cfg.default_profile);
            })
        };

        {
            let s = Rc::clone(&sync_and_save);
            color_row.connect_selected_notify(move |_| s());
        }
        {
            let s = Rc::clone(&sync_and_save);
            shape_row.connect_selected_notify(move |_| s());
        }
        {
            let s = Rc::clone(&sync_and_save);
            blink_row.connect_selected_notify(move |_| s());
        }
        {
            let s = Rc::clone(&sync_and_save);
            font_entry.connect_changed(move |_| s());
        }

        Self { window }
    }

    pub fn present(&self) {
        self.window.present();
    }
}
