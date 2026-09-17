use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;

use crate::model::config::{AppConfig, PaneTitleStyle, WindowStyle};
use crate::model::keybindings::{ActionCategory, ActionShortcutDef, ACTION_CATALOG};
use crate::model::profile::{CursorBlinkPreference, CursorShapePreference, Profile};
use crate::model::theme::ColorScheme;


#[allow(deprecated)]
pub struct TilixPreferencesWindow {
    window: adw::PreferencesWindow,
}

#[allow(deprecated)]
impl TilixPreferencesWindow {
    pub fn new<W: IsA<gtk::Window>, F: Fn(&Profile) + 'static>(
        parent: Option<&W>,
        on_profile_changed: F,
    ) -> Self {
        let window = adw::PreferencesWindow::new();
        if let Some(parent) = parent {
            window.set_transient_for(Some(parent));
            window.set_modal(true);
        } else {
            window.set_modal(false);
        }
        window.set_title(Some("Preferences"));
        window.set_default_size(700, 600);

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

        // Shortcuts Page
        let shortcuts_page = adw::PreferencesPage::new();
        shortcuts_page.set_title("Shortcuts");
        shortcuts_page.set_icon_name(Some("preferences-desktop-keyboard-shortcuts-symbolic"));

        #[derive(Clone)]
        struct ShortcutRowWidgets {
            action_id: &'static str,
            shortcut_label: gtk::ShortcutLabel,
            disabled_label: gtk::Label,
            reset_btn: gtk::Button,
        }

        let row_widgets = Rc::new(RefCell::new(Vec::<ShortcutRowWidgets>::new()));

        let refresh_shortcuts_ui = {
            let current_config = Rc::clone(&current_config);
            let row_widgets = Rc::clone(&row_widgets);
            Rc::new(move || {
                let cfg = current_config.borrow();
                for item in row_widgets.borrow().iter() {
                    let effective = cfg
                        .keybindings
                        .get_effective_accel(item.action_id)
                        .unwrap_or_default();
                    let is_custom = cfg.keybindings.is_customized(item.action_id);
                    if effective.trim().is_empty() {
                        item.shortcut_label.set_visible(false);
                        item.disabled_label.set_visible(true);
                    } else {
                        item.shortcut_label.set_accelerator(&effective);
                        item.shortcut_label.set_visible(true);
                        item.disabled_label.set_visible(false);
                    }
                    item.reset_btn.set_visible(is_custom);
                }
            })
        };

        // Top Defaults Group
        let top_group = adw::PreferencesGroup::new();
        top_group.set_title("Defaults");
        let reset_all_row = adw::ActionRow::new();
        reset_all_row.set_title("Reset All Keybindings");
        reset_all_row.set_subtitle("Restore all keyboard shortcuts to application default values");
        let reset_all_btn = gtk::Button::with_label("Reset All");
        reset_all_btn.add_css_class("destructive-action");
        reset_all_btn.set_valign(gtk::Align::Center);
        reset_all_row.add_suffix(&reset_all_btn);
        top_group.add(&reset_all_row);
        shortcuts_page.add(&top_group);

        {
            let current_config = Rc::clone(&current_config);
            let refresh = Rc::clone(&refresh_shortcuts_ui);
            reset_all_btn.connect_clicked(move |_| {
                current_config.borrow_mut().keybindings.reset_all();
                let _ = current_config.borrow().save();
                crate::ui::window::apply_keybindings_globally(&current_config.borrow().keybindings);
                refresh();
            });
        }

        // Category Groups
        let categories = [
            ActionCategory::SessionAndTabs,
            ActionCategory::SplitsAndLayout,
            ActionCategory::Navigation,
            ActionCategory::ViewAndSettings,
        ];

        for category in categories {
            let cat_group = adw::PreferencesGroup::new();
            cat_group.set_title(&glib::markup_escape_text(category.title()));

            for def in ACTION_CATALOG.iter().filter(|d| d.category == category) {
                let row = adw::ActionRow::new();
                row.set_title(def.title);
                row.set_subtitle(def.description);
                row.set_activatable(true);

                let suffix_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                suffix_box.set_valign(gtk::Align::Center);

                let shortcut_label = gtk::ShortcutLabel::new("");
                let disabled_label = gtk::Label::new(Some("Disabled"));
                disabled_label.add_css_class("dim-label");

                let edit_btn = gtk::Button::from_icon_name("document-edit-symbolic");
                edit_btn.set_tooltip_text(Some("Edit shortcut"));
                edit_btn.add_css_class("flat");

                let reset_btn = gtk::Button::from_icon_name("edit-undo-symbolic");
                reset_btn.set_tooltip_text(Some("Reset to default"));
                reset_btn.add_css_class("flat");

                suffix_box.append(&shortcut_label);
                suffix_box.append(&disabled_label);
                suffix_box.append(&edit_btn);
                suffix_box.append(&reset_btn);
                row.add_suffix(&suffix_box);

                row_widgets.borrow_mut().push(ShortcutRowWidgets {
                    action_id: def.id,
                    shortcut_label,
                    disabled_label,
                    reset_btn: reset_btn.clone(),
                });

                // Reset button callback
                {
                    let current_config = Rc::clone(&current_config);
                    let refresh = Rc::clone(&refresh_shortcuts_ui);
                    let action_id = def.id;
                    reset_btn.connect_clicked(move |_| {
                        current_config.borrow_mut().keybindings.reset_action(action_id);
                        let _ = current_config.borrow().save();
                        crate::ui::window::apply_keybindings_globally(
                            &current_config.borrow().keybindings,
                        );
                        refresh();
                    });
                }

                // Edit button and row activation callback
                let open_dialog = {
                    let win_weak = window.downgrade();
                    let current_config = Rc::clone(&current_config);
                    let refresh = Rc::clone(&refresh_shortcuts_ui);
                    let action_def = def;
                    move || {
                        let Some(parent_win) = win_weak.upgrade() else { return; };
                        let effective = current_config
                            .borrow()
                            .keybindings
                            .get_effective_accel(action_def.id)
                            .unwrap_or_default();
                        let config_clone = Rc::clone(&current_config);
                        let refresh_clone = Rc::clone(&refresh);
                        let action_id = action_def.id;
                        let dlg = ShortcutCaptureDialog::new(
                            &parent_win,
                            action_def,
                            &effective,
                            Rc::clone(&current_config),
                            move |new_accel| {
                                config_clone
                                    .borrow_mut()
                                    .keybindings
                                    .set_custom_accel(action_id, new_accel);
                                let _ = config_clone.borrow().save();
                                crate::ui::window::apply_keybindings_globally(
                                    &config_clone.borrow().keybindings,
                                );
                                refresh_clone();
                            },
                        );
                        dlg.present();
                    }
                };

                {
                    let od = open_dialog.clone();
                    edit_btn.connect_clicked(move |_| od());
                }
                {
                    let od = open_dialog;
                    row.connect_activated(move |_| od());
                }

                cat_group.add(&row);
            }

            shortcuts_page.add(&cat_group);
        }

        refresh_shortcuts_ui();
        window.add(&shortcuts_page);

        Self { window }
    }

    pub fn window(&self) -> &adw::PreferencesWindow {
        &self.window
    }

    pub fn present(&self) {
        self.window.present();
    }
}

#[allow(deprecated)]
pub struct ShortcutCaptureDialog {
    window: adw::Window,
}

#[allow(deprecated)]
impl ShortcutCaptureDialog {
    pub fn new<F: Fn(&str) + 'static>(

        parent: &impl IsA<gtk::Window>,
        def: &'static ActionShortcutDef,
        current_accel: &str,
        config: Rc<RefCell<AppConfig>>,
        on_apply: F,
    ) -> Self {
        let window = adw::Window::builder()
            .title(format!("Set Shortcut — {}", def.title))
            .modal(true)

            .transient_for(parent)
            .destroy_with_parent(true)
            .default_width(440)
            .default_height(260)
            .build();

        let header_bar = adw::HeaderBar::new();
        header_bar.set_show_end_title_buttons(false);
        header_bar.set_show_start_title_buttons(false);

        let cancel_btn = gtk::Button::with_label("Cancel");
        header_bar.pack_start(&cancel_btn);

        let apply_btn = gtk::Button::with_label("Set");
        apply_btn.add_css_class("suggested-action");
        header_bar.pack_end(&apply_btn);

        let title_widget = adw::WindowTitle::new(
            &format!("Set Shortcut: {}", def.title),
            def.description,
        );
        header_bar.set_title_widget(Some(&title_widget));

        let main_box = gtk::Box::new(gtk::Orientation::Vertical, 16);
        main_box.set_margin_top(16);
        main_box.set_margin_bottom(16);
        main_box.set_margin_start(20);
        main_box.set_margin_end(20);

        let banner = adw::Banner::new("");
        banner.set_revealed(false);
        main_box.append(&banner);

        let hint = gtk::Label::new(Some(
            "Press the desired key combination\n(Escape cancels, Backspace/Delete unbinds)",
        ));
        hint.set_justify(gtk::Justification::Center);
        hint.add_css_class("dim-label");
        main_box.append(&hint);

        let display_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        display_box.set_halign(gtk::Align::Center);
        display_box.set_valign(gtk::Align::Center);

        let shortcut_label = gtk::ShortcutLabel::new("");
        let disabled_label = gtk::Label::new(Some("Disabled"));
        disabled_label.add_css_class("dim-label");
        display_box.append(&shortcut_label);
        display_box.append(&disabled_label);
        main_box.append(&display_box);

        let disable_btn = gtk::Button::with_label("Disable Shortcut");
        disable_btn.add_css_class("flat");
        disable_btn.set_halign(gtk::Align::Center);
        main_box.append(&disable_btn);

        let content_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content_box.append(&header_bar);
        content_box.append(&main_box);
        window.set_content(Some(&content_box));

        let captured_accel = Rc::new(RefCell::new(current_accel.to_string()));

        let update_preview = {
            let captured_accel = Rc::clone(&captured_accel);
            let shortcut_label = shortcut_label.clone();
            let disabled_label = disabled_label.clone();
            let banner = banner.clone();
            let config = Rc::clone(&config);
            let action_id = def.id;
            Rc::new(move || {
                let accel = captured_accel.borrow().clone();
                if accel.trim().is_empty() {
                    shortcut_label.set_visible(false);
                    disabled_label.set_visible(true);
                    banner.set_revealed(false);
                } else {
                    shortcut_label.set_accelerator(&accel);
                    shortcut_label.set_visible(true);
                    disabled_label.set_visible(false);

                    let conflict = config.borrow().keybindings.check_conflict(action_id, &accel);
                    if let Some(c) = conflict {
                        banner.set_title(&format!(
                            "⚠️ Already assigned to '{}' ({})",
                            glib::markup_escape_text(&c.action_title),
                            glib::markup_escape_text(&c.conflicting_accel)
                        ));
                        banner.set_revealed(true);
                    } else {
                        banner.set_revealed(false);
                    }
                }
            })
        };

        update_preview();

        // Key Controller
        let controller = gtk::EventControllerKey::new();
        {
            let captured_accel = Rc::clone(&captured_accel);
            let update_preview = Rc::clone(&update_preview);
            let win_weak = window.downgrade();
            controller.connect_key_pressed(move |_, keyval, _keycode, state| {
                match keyval {
                    gtk::gdk::Key::Shift_L
                    | gtk::gdk::Key::Shift_R
                    | gtk::gdk::Key::Control_L
                    | gtk::gdk::Key::Control_R
                    | gtk::gdk::Key::Alt_L
                    | gtk::gdk::Key::Alt_R
                    | gtk::gdk::Key::Super_L
                    | gtk::gdk::Key::Super_R
                    | gtk::gdk::Key::Meta_L
                    | gtk::gdk::Key::Meta_R => return glib::Propagation::Proceed,
                    _ => {}
                }

                if keyval == gtk::gdk::Key::Escape {
                    if let Some(win) = win_weak.upgrade() {
                        win.close();
                    }
                    return glib::Propagation::Stop;
                }

                let is_ctrl = state.contains(gtk::gdk::ModifierType::CONTROL_MASK);
                let is_alt = state.contains(gtk::gdk::ModifierType::ALT_MASK);
                let is_super = state.contains(gtk::gdk::ModifierType::SUPER_MASK);

                if (keyval == gtk::gdk::Key::BackSpace || keyval == gtk::gdk::Key::Delete)
                    && !is_ctrl
                    && !is_alt
                    && !is_super
                {
                    *captured_accel.borrow_mut() = String::new();
                    update_preview();
                    return glib::Propagation::Stop;
                }

                let mods = state
                    & (gtk::gdk::ModifierType::CONTROL_MASK
                        | gtk::gdk::ModifierType::SHIFT_MASK
                        | gtk::gdk::ModifierType::ALT_MASK
                        | gtk::gdk::ModifierType::SUPER_MASK);
                let raw_name = gtk::accelerator_name(keyval, mods);
                let normalized = crate::model::keybindings::normalize_accelerator(&raw_name);
                if !normalized.is_empty() {
                    *captured_accel.borrow_mut() = normalized;
                    update_preview();
                }

                glib::Propagation::Stop
            });
        }
        window.add_controller(controller);

        // Cancel
        {
            let win_weak = window.downgrade();
            cancel_btn.connect_clicked(move |_| {
                if let Some(win) = win_weak.upgrade() {
                    win.close();
                }
            });
        }

        // Disable
        let on_apply = Rc::new(on_apply);
        {
            let win_weak = window.downgrade();
            let cb = Rc::clone(&on_apply);
            disable_btn.connect_clicked(move |_| {
                cb("");
                if let Some(win) = win_weak.upgrade() {
                    win.close();
                }
            });
        }

        // Apply
        {
            let win_weak = window.downgrade();
            let cb = Rc::clone(&on_apply);
            let captured_accel = Rc::clone(&captured_accel);
            apply_btn.connect_clicked(move |_| {
                let accel = captured_accel.borrow().clone();
                cb(&accel);
                if let Some(win) = win_weak.upgrade() {
                    win.close();
                }
            });
        }

        Self { window }
    }

    pub fn present(&self) {
        self.window.present();
    }
}

