use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;

use crate::model::config::{AppConfig, PaneTitleStyle, WindowStyle};
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

        // Window Group
        let window_group = adw::PreferencesGroup::new();
        window_group.set_title("Window");

        let window_style_names = ["Normal", "Hide Toolbar"];
        let window_style_model = gtk::StringList::new(&window_style_names);
        let window_style_row = adw::ComboRow::new();
        window_style_row.set_title("Window Style");
        window_style_row.set_model(Some(&window_style_model));
        let style_idx = match current_config.borrow().window_style {
            WindowStyle::Normal => 0,
            WindowStyle::HideToolbar => 1,
        };
        window_style_row.set_selected(style_idx);
        window_group.add(&window_style_row);

        let wide_handle_row = adw::SwitchRow::new();
        wide_handle_row.set_title("Use a wide handle for splitters");
        wide_handle_row.set_subtitle("Increase draggable handle size between split panes");
        wide_handle_row.set_active(current_config.borrow().use_wide_handle);
        window_group.add(&wide_handle_row);

        let show_tab_bar_row = adw::SwitchRow::new();
        show_tab_bar_row.set_title("Show Tab Bar");
        show_tab_bar_row.set_subtitle("Show or hide the tab bar (toggle with F12 or Ctrl+Shift+F12)");
        show_tab_bar_row.set_active(current_config.borrow().show_tab_bar);
        window_group.add(&show_tab_bar_row);

        page.add(&window_group);

        // Terminal Title Group
        let title_group = adw::PreferencesGroup::new();
        title_group.set_title("Terminal Title");

        let title_style_names = ["Normal", "None"];
        let title_style_model = gtk::StringList::new(&title_style_names);
        let title_style_row = adw::ComboRow::new();
        title_style_row.set_title("Title Style");
        title_style_row.set_model(Some(&title_style_model));
        let title_idx = match current_config.borrow().pane_title_style {
            PaneTitleStyle::Normal => 0,
            PaneTitleStyle::None => 1,
        };
        title_style_row.set_selected(title_idx);
        title_group.add(&title_style_row);

        let title_show_single_row = adw::SwitchRow::new();
        title_show_single_row.set_title("Show title when single terminal");
        title_show_single_row.set_subtitle("Display terminal pane header bar even when no splits exist");
        title_show_single_row.set_active(current_config.borrow().pane_title_show_when_single);
        title_group.add(&title_show_single_row);

        page.add(&title_group);

        window.add(&page);

        // Behavior Page
        let behavior_page = adw::PreferencesPage::new();
        behavior_page.set_title("Behavior");
        behavior_page.set_icon_name(Some("preferences-system-symbolic"));

        // Quake Group
        let quake_group = adw::PreferencesGroup::new();
        quake_group.set_title("Quake Drop-Down");
        behavior_page.add(&quake_group);

        let quake_height_adj = gtk::Adjustment::new(
            current_config.borrow().quake_height_percent as f64,
            10.0,
            100.0,
            5.0,
            10.0,
            0.0,
        );
        let quake_height_row = adw::SpinRow::new(Some(&quake_height_adj), 1.0, 0);
        quake_height_row.set_title("Quake Height Percentage");
        quake_group.add(&quake_height_row);

        let quake_unfocus_row = adw::SwitchRow::new();
        quake_unfocus_row.set_title("Hide Quake on Focus Loss");
        quake_unfocus_row.set_active(current_config.borrow().quake_hide_on_unfocus);
        quake_group.add(&quake_unfocus_row);

        // Notifications Group
        let notif_group = adw::PreferencesGroup::new();
        notif_group.set_title("Notifications");
        behavior_page.add(&notif_group);

        let notif_enabled_row = adw::SwitchRow::new();
        notif_enabled_row.set_title("Enable Notifications");
        notif_enabled_row.set_active(current_config.borrow().notifications_enabled);
        notif_group.add(&notif_enabled_row);

        let notif_bell_row = adw::SwitchRow::new();
        notif_bell_row.set_title("Terminal Bell Notifications");
        notif_bell_row.set_active(current_config.borrow().bell_notifications);
        notif_group.add(&notif_bell_row);

        let notif_exit_row = adw::SwitchRow::new();
        notif_exit_row.set_title("Process Exit Notifications");
        notif_exit_row.set_active(current_config.borrow().process_exit_notifications);
        notif_group.add(&notif_exit_row);

        window.add(&behavior_page);

        // Helper to apply and save changes
        let sync_and_save = {
            let config_rc = Rc::clone(&current_config);
            let on_change_cb = Rc::clone(&on_change);
            let c_row = color_row.clone();
            let s_row = shape_row.clone();
            let b_row = blink_row.clone();
            let f_entry = font_entry.clone();
            let q_height = quake_height_row.clone();
            let q_unfocus = quake_unfocus_row.clone();
            let n_enabled = notif_enabled_row.clone();
            let n_bell = notif_bell_row.clone();
            let n_exit = notif_exit_row.clone();
            let w_style = window_style_row.clone();
            let w_handle = wide_handle_row.clone();
            let t_bar = show_tab_bar_row.clone();
            let t_style = title_style_row.clone();
            let t_single = title_show_single_row.clone();

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

                let window_style = match w_style.selected() {
                    1 => WindowStyle::HideToolbar,
                    _ => WindowStyle::Normal,
                };
                let use_wide_handle = w_handle.is_active();
                let show_tab_bar = t_bar.is_active();

                let pane_title_style = match t_style.selected() {
                    1 => PaneTitleStyle::None,
                    _ => PaneTitleStyle::Normal,
                };
                let pane_title_show_when_single = t_single.is_active();

                let mut cfg = config_rc.borrow_mut();
                cfg.default_profile.color_scheme = color_scheme;
                cfg.default_profile.cursor_shape = cursor_shape;
                cfg.default_profile.cursor_blink = cursor_blink;
                cfg.default_profile.font = font;

                cfg.quake_height_percent = q_height.value() as u32;
                cfg.quake_hide_on_unfocus = q_unfocus.is_active();
                cfg.notifications_enabled = n_enabled.is_active();
                cfg.bell_notifications = n_bell.is_active();
                cfg.process_exit_notifications = n_exit.is_active();
                cfg.window_style = window_style;
                cfg.use_wide_handle = use_wide_handle;
                cfg.pane_title_style = pane_title_style;
                cfg.pane_title_show_when_single = pane_title_show_when_single;
                cfg.show_tab_bar = show_tab_bar;

                let _ = cfg.save();
                crate::ui::window::apply_window_style_to_all_windows(cfg.window_style);
                crate::ui::window::apply_wide_handle_to_all_sessions(cfg.use_wide_handle);
                crate::ui::window::apply_show_tab_bar_to_all_windows(cfg.show_tab_bar);
                crate::ui::window::apply_pane_title_settings_to_all_sessions(
                    cfg.pane_title_style,
                    cfg.pane_title_show_when_single,
                );
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
        {
            let s = Rc::clone(&sync_and_save);
            quake_height_row.connect_changed(move |_| s());
        }
        {
            let s = Rc::clone(&sync_and_save);
            quake_unfocus_row.connect_active_notify(move |_| s());
        }
        {
            let s = Rc::clone(&sync_and_save);
            notif_enabled_row.connect_active_notify(move |_| s());
        }
        {
            let s = Rc::clone(&sync_and_save);
            notif_bell_row.connect_active_notify(move |_| s());
        }
        {
            let s = Rc::clone(&sync_and_save);
            notif_exit_row.connect_active_notify(move |_| s());
        }
        {
            let s = Rc::clone(&sync_and_save);
            window_style_row.connect_selected_notify(move |_| s());
        }
        {
            let s = Rc::clone(&sync_and_save);
            wide_handle_row.connect_active_notify(move |_| s());
        }
        {
            let s = Rc::clone(&sync_and_save);
            show_tab_bar_row.connect_active_notify(move |_| s());
        }
        {
            let s = Rc::clone(&sync_and_save);
            title_style_row.connect_selected_notify(move |_| s());
        }
        {
            let s = Rc::clone(&sync_and_save);
            title_show_single_row.connect_active_notify(move |_| s());
        }

        Self { window }
    }

    pub fn present(&self) {
        self.window.present();
    }
}
