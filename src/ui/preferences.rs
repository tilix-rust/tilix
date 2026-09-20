use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;

use crate::model::config::{AppConfig, PaneTitleStyle, WindowStyle};
use crate::model::keybindings::{ActionCategory, ActionShortcutDef, ACTION_CATALOG};
use crate::model::profile::{
    BadgePosition, CjkWidthPreference, CursorBlinkPreference, CursorShapePreference,
    CustomHyperlinkRule, EraseBindingPreference, ExitActionPreference, Profile, ProfileSwitchRule,
    TerminalBellPreference, TextBlinkModePreference,
};
use crate::model::theme::{ColorScheme, RgbColor};
use crate::model::title::{
    TitleEditScope, SESSION_TOKEN_DEFS, TERMINAL_TOKEN_DEFS, WINDOW_TOKEN_DEFS,
};

fn rgb_to_rgba(c: &RgbColor) -> gtk::gdk::RGBA {
    gtk::gdk::RGBA::builder()
        .red(c.red as f32)
        .green(c.green as f32)
        .blue(c.blue as f32)
        .alpha(c.alpha as f32)
        .build()
}

fn rgba_to_rgb(rgba: &gtk::gdk::RGBA) -> RgbColor {
    RgbColor::new(
        rgba.red() as f64,
        rgba.green() as f64,
        rgba.blue() as f64,
        rgba.alpha() as f64,
    )
}

pub fn create_scoped_token_menu_button(
    target_entry: &gtk::Entry,
    scope: TitleEditScope,
) -> gtk::MenuButton {
    let menu_btn = gtk::MenuButton::new();
    menu_btn.set_icon_name("pan-down-symbolic");

    let popover = gtk::Popover::new();
    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 4);
    vbox.set_margin_top(6);
    vbox.set_margin_bottom(6);
    vbox.set_margin_start(6);
    vbox.set_margin_end(6);

    let add_header = |v: &gtk::Box, title: &str| {
        let lbl = gtk::Label::new(Some(title));
        lbl.add_css_class("heading");
        lbl.set_halign(gtk::Align::Start);
        lbl.set_margin_top(4);
        lbl.set_margin_bottom(2);
        v.append(&lbl);
    };

    let add_button = |v: &gtk::Box, tok: &'static str, desc: &'static str| {
        let b = gtk::Button::with_label(tok);
        b.add_css_class("flat");
        b.set_halign(gtk::Align::Start);
        b.set_tooltip_text(Some(desc));
        let entry_clone = target_entry.clone();
        let pop_weak = popover.downgrade();
        b.connect_clicked(move |_| {
            let mut pos = entry_clone.position();
            entry_clone.insert_text(tok, &mut pos);
            entry_clone.set_position(pos);
            if let Some(p) = pop_weak.upgrade() {
                p.popdown();
            }
            entry_clone.grab_focus();
        });
        v.append(&b);
    };

    // 1. Terminal section (always visible)
    add_header(&vbox, "Terminal");
    for def in TERMINAL_TOKEN_DEFS {
        add_button(&vbox, def.token, def.description);
    }

    // 2. Session section (visible for Session and Window scopes)
    if scope == TitleEditScope::Session || scope == TitleEditScope::Window {
        let sep = gtk::Separator::new(gtk::Orientation::Horizontal);
        vbox.append(&sep);
        add_header(&vbox, "Session");
        for def in SESSION_TOKEN_DEFS {
            add_button(&vbox, def.token, def.description);
        }
    }

    // 3. Window section (visible for Window scope)
    if scope == TitleEditScope::Window {
        let sep = gtk::Separator::new(gtk::Orientation::Horizontal);
        vbox.append(&sep);
        add_header(&vbox, "Window");
        for def in WINDOW_TOKEN_DEFS {
            add_button(&vbox, def.token, def.description);
        }
    }

    // 4. Help section
    let sep = gtk::Separator::new(gtk::Orientation::Horizontal);
    vbox.append(&sep);
    let help_btn = gtk::Button::with_label("Online Help...");
    help_btn.add_css_class("flat");
    help_btn.set_halign(gtk::Align::Start);
    help_btn.set_icon_name("help-browser-symbolic");
    let pop_weak_help = popover.downgrade();
    help_btn.connect_clicked(move |_| {
        let _ = gio::AppInfo::launch_default_for_uri(
            "https://gnunn1.github.io/tilix-web/manual/title/",
            None::<&gio::AppLaunchContext>,
        );
        if let Some(p) = pop_weak_help.upgrade() {
            p.popdown();
        }
    });
    vbox.append(&help_btn);

    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .max_content_height(350)
        .propagate_natural_height(true)
        .child(&vbox)
        .build();

    popover.set_child(Some(&scrolled));
    menu_btn.set_popover(Some(&popover));
    menu_btn
}

pub fn create_token_menu_button(target_entry: &gtk::Entry) -> gtk::MenuButton {
    create_scoped_token_menu_button(target_entry, TitleEditScope::Terminal)
}

fn create_color_button() -> gtk::ColorDialogButton {
    let dialog = gtk::ColorDialog::builder().with_alpha(false).build();
    gtk::ColorDialogButton::new(Some(dialog))
}

fn create_font_button() -> gtk::FontDialogButton {
    let dialog = gtk::FontDialog::new();
    gtk::FontDialogButton::new(Some(dialog))
}

fn show_auto_switch_rule_dialog<W: IsA<gtk::Window>, F: Fn(ProfileSwitchRule) + 'static>(
    parent: &W,
    existing: Option<&ProfileSwitchRule>,
    profile_id: String,
    on_save: F,
) {
    let dialog = gtk::Window::builder()
        .title(if existing.is_some() { "Edit Rule" } else { "Add Rule" })
        .modal(true)
        .transient_for(parent)
        .destroy_with_parent(true)
        .default_width(380)
        .default_height(200)
        .build();

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 12);
    vbox.set_margin_top(16);
    vbox.set_margin_bottom(16);
    vbox.set_margin_start(16);
    vbox.set_margin_end(16);

    let grid = gtk::Grid::new();
    grid.set_column_spacing(12);
    grid.set_row_spacing(8);

    let host_lbl = gtk::Label::new(Some("Hostname"));
    host_lbl.set_halign(gtk::Align::End);
    let host_entry = gtk::Entry::new();
    host_entry.set_hexpand(true);
    if let Some(r) = existing {
        host_entry.set_text(&r.hostname);
    }

    let dir_lbl = gtk::Label::new(Some("Directory"));
    dir_lbl.set_halign(gtk::Align::End);
    let dir_entry = gtk::Entry::new();
    dir_entry.set_hexpand(true);
    if let Some(r) = existing {
        dir_entry.set_text(&r.directory);
    }

    grid.attach(&host_lbl, 0, 0, 1, 1);
    grid.attach(&host_entry, 1, 0, 1, 1);
    grid.attach(&dir_lbl, 0, 1, 1, 1);
    grid.attach(&dir_entry, 1, 1, 1, 1);
    vbox.append(&grid);

    let hint = gtk::Label::new(Some(
        "Enter hostname, directory, or both.\nFormat: hostname:directory",
    ));
    hint.add_css_class("dim-label");
    vbox.append(&hint);

    let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    btn_box.set_halign(gtk::Align::End);

    let cancel_btn = gtk::Button::with_label("Cancel");
    let save_btn = gtk::Button::with_label("Save");
    save_btn.add_css_class("suggested-action");

    btn_box.append(&cancel_btn);
    btn_box.append(&save_btn);
    vbox.append(&btn_box);

    dialog.set_child(Some(&vbox));

    let win_weak = dialog.downgrade();
    cancel_btn.connect_clicked(move |_| {
        if let Some(w) = win_weak.upgrade() {
            w.close();
        }
    });

    let win_weak = dialog.downgrade();
    let on_save = Rc::new(on_save);
    save_btn.connect_clicked(move |_| {
        let h = host_entry.text().to_string().trim().to_string();
        let d = dir_entry.text().to_string().trim().to_string();
        if h.is_empty() && d.is_empty() {
            return;
        }
        on_save(ProfileSwitchRule {
            hostname: h,
            directory: d,
            profile_id: profile_id.clone(),
        });
        if let Some(w) = win_weak.upgrade() {
            w.close();
        }
    });

    dialog.present();
}

fn show_custom_links_dialog<W: IsA<gtk::Window>, F: Fn(Vec<CustomHyperlinkRule>) + 'static>(
    parent: &W,
    initial_links: Vec<CustomHyperlinkRule>,
    on_save: F,
) {
    let dialog = gtk::Window::builder()
        .title("Custom Links")
        .modal(true)
        .transient_for(parent)
        .destroy_with_parent(true)
        .default_width(480)
        .default_height(360)
        .build();

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 10);
    vbox.set_margin_top(12);
    vbox.set_margin_bottom(12);
    vbox.set_margin_start(16);
    vbox.set_margin_end(16);

    let links_rc = Rc::new(RefCell::new(initial_links));
    let list_box = gtk::ListBox::new();
    list_box.set_selection_mode(gtk::SelectionMode::Single);

    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_child(Some(&list_box));
    let frame = gtk::Frame::new(None);
    frame.set_child(Some(&scrolled));
    vbox.append(&frame);

    let refresh_list = {
        let links_rc = Rc::clone(&links_rc);
        let list_box = list_box.clone();
        Rc::new(move || {
            while let Some(child) = list_box.first_child() {
                list_box.remove(&child);
            }
            for link in links_rc.borrow().iter() {
                let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                row_box.set_margin_top(4);
                row_box.set_margin_bottom(4);
                row_box.set_margin_start(8);
                row_box.set_margin_end(8);

                let name_lbl = gtk::Label::new(Some(&link.name));
                name_lbl.add_css_class("heading");
                let pat_lbl = gtk::Label::new(Some(&format!("({})", link.pattern)));
                pat_lbl.add_css_class("dim-label");

                row_box.append(&name_lbl);
                row_box.append(&pat_lbl);
                list_box.append(&row_box);
            }
        })
    };
    refresh_list();

    let btn_bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    btn_bar.set_halign(gtk::Align::End);

    let add_btn = gtk::Button::with_label("Add");
    let del_btn = gtk::Button::with_label("Delete");
    let close_btn = gtk::Button::with_label("Close");
    close_btn.add_css_class("suggested-action");

    btn_bar.append(&add_btn);
    btn_bar.append(&del_btn);
    btn_bar.append(&close_btn);
    vbox.append(&btn_bar);

    dialog.set_child(Some(&vbox));

    // Delete
    {
        let links_rc = Rc::clone(&links_rc);
        let list_box = list_box.clone();
        let refresh_list = Rc::clone(&refresh_list);
        del_btn.connect_clicked(move |_| {
            if let Some(row) = list_box.selected_row() {
                let idx = row.index() as usize;
                let mut links = links_rc.borrow_mut();
                if idx < links.len() {
                    links.remove(idx);
                    drop(links);
                    refresh_list();
                }
            }
        });
    }

    // Add
    {
        let links_rc = Rc::clone(&links_rc);
        let refresh_list = Rc::clone(&refresh_list);
        let dia_weak = dialog.downgrade();
        add_btn.connect_clicked(move |_| {
            let Some(parent_win) = dia_weak.upgrade() else { return; };
            show_edit_link_dialog(&parent_win, None, {
                let links_rc = Rc::clone(&links_rc);
                let refresh_list = Rc::clone(&refresh_list);
                move |new_link| {
                    links_rc.borrow_mut().push(new_link);
                    refresh_list();
                }
            });
        });
    }

    // Close
    {
        let win_weak = dialog.downgrade();
        let links_rc = Rc::clone(&links_rc);
        close_btn.connect_clicked(move |_| {
            let final_links = links_rc.borrow().clone();
            on_save(final_links);
            if let Some(w) = win_weak.upgrade() {
                w.close();
            }
        });
    }

    dialog.present();
}

fn show_edit_link_dialog<W: IsA<gtk::Window>, F: Fn(CustomHyperlinkRule) + 'static>(
    parent: &W,
    existing: Option<&CustomHyperlinkRule>,
    on_save: F,
) {
    let dialog = gtk::Window::builder()
        .title(if existing.is_some() { "Edit Link" } else { "Add Link" })
        .modal(true)
        .transient_for(parent)
        .destroy_with_parent(true)
        .default_width(360)
        .default_height(200)
        .build();

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 10);
    vbox.set_margin_top(12);
    vbox.set_margin_bottom(12);
    vbox.set_margin_start(16);
    vbox.set_margin_end(16);

    let grid = gtk::Grid::new();
    grid.set_column_spacing(10);
    grid.set_row_spacing(8);

    let name_lbl = gtk::Label::new(Some("Name"));
    name_lbl.set_halign(gtk::Align::End);
    let name_entry = gtk::Entry::new();
    name_entry.set_hexpand(true);
    if let Some(e) = existing {
        name_entry.set_text(&e.name);
    }

    let pat_lbl = gtk::Label::new(Some("Pattern (regex)"));
    pat_lbl.set_halign(gtk::Align::End);
    let pat_entry = gtk::Entry::new();
    pat_entry.set_hexpand(true);
    if let Some(e) = existing {
        pat_entry.set_text(&e.pattern);
    }

    let uri_lbl = gtk::Label::new(Some("URI"));
    uri_lbl.set_halign(gtk::Align::End);
    let uri_entry = gtk::Entry::new();
    uri_entry.set_hexpand(true);
    if let Some(e) = existing {
        uri_entry.set_text(&e.uri);
    }

    grid.attach(&name_lbl, 0, 0, 1, 1);
    grid.attach(&name_entry, 1, 0, 1, 1);
    grid.attach(&pat_lbl, 0, 1, 1, 1);
    grid.attach(&pat_entry, 1, 1, 1, 1);
    grid.attach(&uri_lbl, 0, 2, 1, 1);
    grid.attach(&uri_entry, 1, 2, 1, 1);
    vbox.append(&grid);

    let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    btn_box.set_halign(gtk::Align::End);
    let cancel_btn = gtk::Button::with_label("Cancel");
    let save_btn = gtk::Button::with_label("Save");
    save_btn.add_css_class("suggested-action");
    btn_box.append(&cancel_btn);
    btn_box.append(&save_btn);
    vbox.append(&btn_box);

    dialog.set_child(Some(&vbox));

    let win_weak = dialog.downgrade();
    cancel_btn.connect_clicked(move |_| {
        if let Some(w) = win_weak.upgrade() {
            w.close();
        }
    });

    let win_weak = dialog.downgrade();
    save_btn.connect_clicked(move |_| {
        let n = name_entry.text().to_string().trim().to_string();
        let p = pat_entry.text().to_string().trim().to_string();
        let u = uri_entry.text().to_string().trim().to_string();
        if !n.is_empty() && !p.is_empty() {
            on_save(CustomHyperlinkRule {
                name: n,
                pattern: p,
                uri: u,
            });
            if let Some(w) = win_weak.upgrade() {
                w.close();
            }
        }
    });

    dialog.present();
}

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
        window.set_default_size(900, 820);
        window.set_size_request(820, 600);

        let current_config = Rc::new(RefCell::new(AppConfig::load()));
        let on_change = Rc::new(on_profile_changed);

        // =========================================================================
        // PROFILES PAGE (100% UI Parity with Original Tilix)
        // =========================================================================
        let profiles_page = adw::PreferencesPage::new();
        profiles_page.set_title("Profiles");
        profiles_page.set_icon_name(Some("org.gnome.Settings-symbolic"));

        // Top Profile Management Header Bar
        let profile_header_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        profile_header_box.set_margin_top(8);
        profile_header_box.set_margin_bottom(8);
        profile_header_box.set_margin_start(12);
        profile_header_box.set_margin_end(12);

        let profile_lbl = gtk::Label::new(Some("Profile:"));
        profile_lbl.set_markup("<b>Profile:</b>");
        profile_header_box.append(&profile_lbl);

        let profile_dropdown = gtk::DropDown::new(None::<gtk::StringList>, gtk::Expression::NONE);
        profile_dropdown.set_hexpand(false);
        profile_header_box.append(&profile_dropdown);

        let add_btn = gtk::Button::with_label("New");
        add_btn.set_tooltip_text(Some("Create a new profile"));
        profile_header_box.append(&add_btn);

        let dup_btn = gtk::Button::with_label("Duplicate");
        dup_btn.set_tooltip_text(Some("Duplicate selected profile"));
        profile_header_box.append(&dup_btn);

        let del_btn = gtk::Button::with_label("Delete");
        del_btn.set_tooltip_text(Some("Delete selected profile"));
        del_btn.add_css_class("destructive-action");
        profile_header_box.append(&del_btn);

        let set_def_btn = gtk::Button::with_label("Set as Default");
        set_def_btn.set_tooltip_text(Some("Make this profile the default"));
        profile_header_box.append(&set_def_btn);

        // Main Tab Notebook
        let notebook = gtk::Notebook::new();
        notebook.set_hexpand(true);
        notebook.set_vexpand(true);

        // -------------------------------------------------------------------------
        // 1. General Tab
        // -------------------------------------------------------------------------
        let gen_grid = gtk::Grid::new();
        gen_grid.set_column_spacing(16);
        gen_grid.set_row_spacing(8);
        gen_grid.set_margin_start(24);
        gen_grid.set_margin_end(24);
        gen_grid.set_margin_top(14);
        gen_grid.set_margin_bottom(14);

        // Profile name
        let name_lbl = gtk::Label::new(Some("Profile name"));
        name_lbl.set_halign(gtk::Align::End);
        name_lbl.set_xalign(1.0);
        let name_entry = gtk::Entry::new();
        name_entry.set_hexpand(true);
        gen_grid.attach(&name_lbl, 0, 0, 1, 1);
        gen_grid.attach(&name_entry, 1, 0, 1, 1);

        // Terminal title
        let title_lbl = gtk::Label::new(Some("Terminal title"));
        title_lbl.set_halign(gtk::Align::End);
        title_lbl.set_xalign(1.0);
        let title_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        title_box.add_css_class("linked");
        let title_entry = gtk::Entry::new();
        title_entry.set_hexpand(true);
        let title_token_btn = create_scoped_token_menu_button(&title_entry, TitleEditScope::Terminal);
        title_box.append(&title_entry);
        title_box.append(&title_token_btn);
        gen_grid.attach(&title_lbl, 0, 1, 1, 1);
        gen_grid.attach(&title_box, 1, 1, 1, 1);

        // Section Text Appearance
        let text_app_lbl = gtk::Label::new(None);
        text_app_lbl.set_markup("<b>Text Appearance</b>");
        text_app_lbl.add_css_class("heading");
        text_app_lbl.set_halign(gtk::Align::Start);
        text_app_lbl.set_xalign(0.0);
        text_app_lbl.set_margin_top(12);
        text_app_lbl.set_margin_bottom(4);
        gen_grid.attach(&text_app_lbl, 0, 2, 2, 1);

        // Terminal size
        let size_lbl = gtk::Label::new(Some("Terminal size"));
        size_lbl.set_halign(gtk::Align::End);
        size_lbl.set_xalign(1.0);
        let size_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let cols_adj = gtk::Adjustment::new(80.0, 20.0, 500.0, 1.0, 10.0, 0.0);
        let cols_spin = gtk::SpinButton::new(Some(&cols_adj), 1.0, 0);
        cols_spin.set_size_request(85, -1);
        let cols_lbl = gtk::Label::new(Some("columns"));
        let rows_adj = gtk::Adjustment::new(24.0, 5.0, 200.0, 1.0, 5.0, 0.0);
        let rows_spin = gtk::SpinButton::new(Some(&rows_adj), 1.0, 0);
        rows_spin.set_size_request(85, -1);
        let rows_lbl = gtk::Label::new(Some("rows"));
        let size_reset_btn = gtk::Button::with_label("Reset");
        size_box.append(&cols_spin);
        size_box.append(&cols_lbl);
        size_box.append(&rows_spin);
        size_box.append(&rows_lbl);
        size_box.append(&size_reset_btn);
        gen_grid.attach(&size_lbl, 0, 3, 1, 1);
        gen_grid.attach(&size_box, 1, 3, 1, 1);

        // Cell spacing
        let spacing_lbl = gtk::Label::new(Some("Cell spacing"));
        spacing_lbl.set_halign(gtk::Align::End);
        spacing_lbl.set_xalign(1.0);
        let spacing_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let cell_w_adj = gtk::Adjustment::new(1.0, 0.5, 2.0, 0.1, 0.2, 0.0);
        let cell_w_spin = gtk::SpinButton::new(Some(&cell_w_adj), 0.1, 1);
        cell_w_spin.set_size_request(85, -1);
        let cell_w_lbl = gtk::Label::new(Some("width"));
        let cell_h_adj = gtk::Adjustment::new(1.0, 0.5, 2.0, 0.1, 0.2, 0.0);
        let cell_h_spin = gtk::SpinButton::new(Some(&cell_h_adj), 0.1, 1);
        cell_h_spin.set_size_request(85, -1);
        let cell_h_lbl = gtk::Label::new(Some("height"));
        let spacing_reset_btn = gtk::Button::with_label("Reset");
        spacing_box.append(&cell_w_spin);
        spacing_box.append(&cell_w_lbl);
        spacing_box.append(&cell_h_spin);
        spacing_box.append(&cell_h_lbl);
        spacing_box.append(&spacing_reset_btn);
        gen_grid.attach(&spacing_lbl, 0, 4, 1, 1);
        gen_grid.attach(&spacing_box, 1, 4, 1, 1);

        // Margin
        let margin_lbl = gtk::Label::new(Some("Margin"));
        margin_lbl.set_halign(gtk::Align::End);
        margin_lbl.set_xalign(1.0);
        let margin_adj = gtk::Adjustment::new(80.0, 0.0, 500.0, 1.0, 10.0, 0.0);
        let margin_spin = gtk::SpinButton::new(Some(&margin_adj), 1.0, 0);
        margin_spin.set_halign(gtk::Align::Start);
        margin_spin.set_size_request(120, -1);
        gen_grid.attach(&margin_lbl, 0, 5, 1, 1);
        gen_grid.attach(&margin_spin, 1, 5, 1, 1);

        // Text blink mode
        let blink_lbl = gtk::Label::new(Some("Text blink mode"));
        blink_lbl.set_halign(gtk::Align::End);
        blink_lbl.set_xalign(1.0);
        let blink_names = ["Never", "Focused", "Unfocused", "Always"];
        let blink_model = gtk::StringList::new(&blink_names);
        let blink_combo = gtk::DropDown::new(Some(blink_model), gtk::Expression::NONE);
        blink_combo.set_halign(gtk::Align::Start);
        blink_combo.set_size_request(220, -1);
        gen_grid.attach(&blink_lbl, 0, 6, 1, 1);
        gen_grid.attach(&blink_combo, 1, 6, 1, 1);

        // Custom font
        let custom_font_lbl = gtk::Label::new(Some("Custom font"));
        custom_font_lbl.set_halign(gtk::Align::End);
        custom_font_lbl.set_xalign(1.0);
        let font_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        font_box.set_halign(gtk::Align::Start);
        let custom_font_check = gtk::CheckButton::new();
        let font_btn = create_font_button();
        font_box.append(&custom_font_check);
        font_box.append(&font_btn);
        gen_grid.attach(&custom_font_lbl, 0, 7, 1, 1);
        gen_grid.attach(&font_box, 1, 7, 1, 1);

        // Word-wise select chars
        let word_lbl = gtk::Label::new(Some("Word-wise select chars"));
        word_lbl.set_halign(gtk::Align::End);
        word_lbl.set_xalign(1.0);
        let word_entry = gtk::Entry::new();
        word_entry.set_hexpand(true);
        gen_grid.attach(&word_lbl, 0, 8, 1, 1);
        gen_grid.attach(&word_entry, 1, 8, 1, 1);

        // Section Cursor
        let cursor_sec_lbl = gtk::Label::new(None);
        cursor_sec_lbl.set_markup("<b>Cursor</b>");
        cursor_sec_lbl.add_css_class("heading");
        cursor_sec_lbl.set_halign(gtk::Align::Start);
        cursor_sec_lbl.set_xalign(0.0);
        cursor_sec_lbl.set_margin_top(12);
        cursor_sec_lbl.set_margin_bottom(4);
        gen_grid.attach(&cursor_sec_lbl, 0, 9, 2, 1);

        // Cursor shape
        let cursor_shape_lbl = gtk::Label::new(Some("Cursor"));
        cursor_shape_lbl.set_halign(gtk::Align::End);
        cursor_shape_lbl.set_xalign(1.0);
        let shape_names = ["Block", "I-Beam", "Underline"];
        let shape_model = gtk::StringList::new(&shape_names);
        let cursor_shape_combo = gtk::DropDown::new(Some(shape_model), gtk::Expression::NONE);
        cursor_shape_combo.set_halign(gtk::Align::Start);
        cursor_shape_combo.set_size_request(220, -1);
        gen_grid.attach(&cursor_shape_lbl, 0, 10, 1, 1);
        gen_grid.attach(&cursor_shape_combo, 1, 10, 1, 1);

        // Cursor blink mode
        let cursor_blink_lbl = gtk::Label::new(Some("Cursor blink mode"));
        cursor_blink_lbl.set_halign(gtk::Align::End);
        cursor_blink_lbl.set_xalign(1.0);
        let cblink_names = ["System", "On", "Off"];
        let cblink_model = gtk::StringList::new(&cblink_names);
        let cursor_blink_combo = gtk::DropDown::new(Some(cblink_model), gtk::Expression::NONE);
        cursor_blink_combo.set_halign(gtk::Align::Start);
        cursor_blink_combo.set_size_request(220, -1);
        gen_grid.attach(&cursor_blink_lbl, 0, 11, 1, 1);
        gen_grid.attach(&cursor_blink_combo, 1, 11, 1, 1);

        // Section Notification
        let notif_sec_lbl = gtk::Label::new(None);
        notif_sec_lbl.set_markup("<b>Notification</b>");
        notif_sec_lbl.add_css_class("heading");
        notif_sec_lbl.set_halign(gtk::Align::Start);
        notif_sec_lbl.set_xalign(0.0);
        notif_sec_lbl.set_margin_top(12);
        notif_sec_lbl.set_margin_bottom(4);
        gen_grid.attach(&notif_sec_lbl, 0, 12, 2, 1);

        // Terminal bell
        let bell_lbl = gtk::Label::new(Some("Terminal bell"));
        bell_lbl.set_halign(gtk::Align::End);
        bell_lbl.set_xalign(1.0);
        let bell_names = ["None", "Sound", "Icon", "Icon and sound"];
        let bell_model = gtk::StringList::new(&bell_names);
        let bell_combo = gtk::DropDown::new(Some(bell_model), gtk::Expression::NONE);
        bell_combo.set_halign(gtk::Align::Start);
        bell_combo.set_size_request(220, -1);
        gen_grid.attach(&bell_lbl, 0, 13, 1, 1);
        gen_grid.attach(&bell_combo, 1, 13, 1, 1);

        let copy_on_select_check =
            gtk::CheckButton::with_label("Automatically copy selection to clipboard");
        gen_grid.attach(&copy_on_select_check, 1, 14, 1, 1);

        let gen_scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .propagate_natural_height(true)
            .hexpand(true)
            .vexpand(true)
            .child(&gen_grid)
            .build();
        notebook.append_page(&gen_scrolled, Some(&gtk::Label::new(Some("General"))));

        // -------------------------------------------------------------------------
        // 2. Command Tab
        // -------------------------------------------------------------------------
        let cmd_box = gtk::Box::new(gtk::Orientation::Vertical, 14);
        cmd_box.set_margin_start(20);
        cmd_box.set_margin_end(20);
        cmd_box.set_margin_top(16);
        cmd_box.set_margin_bottom(16);

        let login_shell_check = gtk::CheckButton::with_label("Run command as a login shell");
        let custom_cmd_check = gtk::CheckButton::with_label("Run a custom command instead of my shell");

        let custom_cmd_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        custom_cmd_box.set_margin_start(28);
        let custom_cmd_lbl = gtk::Label::new(Some("Command"));
        let custom_cmd_entry = gtk::Entry::new();
        custom_cmd_entry.set_hexpand(true);
        custom_cmd_box.append(&custom_cmd_lbl);
        custom_cmd_box.append(&custom_cmd_entry);

        let exit_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let exit_lbl = gtk::Label::new(Some("When command exits"));
        let exit_names = ["Exit the terminal", "Restart the command", "Hold the terminal open"];
        let exit_model = gtk::StringList::new(&exit_names);
        let exit_action_combo = gtk::DropDown::new(Some(exit_model), gtk::Expression::NONE);
        exit_action_combo.set_halign(gtk::Align::Start);
        exit_action_combo.set_size_request(240, -1);
        exit_box.append(&exit_lbl);
        exit_box.append(&exit_action_combo);

        cmd_box.append(&login_shell_check);
        cmd_box.append(&custom_cmd_check);
        cmd_box.append(&custom_cmd_box);
        cmd_box.append(&exit_box);

        let cmd_scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .propagate_natural_height(true)
            .hexpand(true)
            .vexpand(true)
            .child(&cmd_box)
            .build();
        notebook.append_page(&cmd_scrolled, Some(&gtk::Label::new(Some("Command"))));

        // -------------------------------------------------------------------------
        // 3. Color Tab
        // -------------------------------------------------------------------------
        let color_box = gtk::Box::new(gtk::Orientation::Vertical, 14);
        color_box.set_margin_start(20);
        color_box.set_margin_end(20);
        color_box.set_margin_top(16);
        color_box.set_margin_bottom(16);

        // Top Row: Color scheme + Export
        let scheme_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let scheme_lbl = gtk::Label::new(None);
        scheme_lbl.set_markup("<b>Color scheme</b>");
        let scheme_names = ["Tilix Dark", "Tilix Light", "Solarized Dark", "Monokai", "Custom"];
        let scheme_model = gtk::StringList::new(&scheme_names);
        let color_scheme_combo = gtk::DropDown::new(Some(scheme_model), gtk::Expression::NONE);
        color_scheme_combo.set_hexpand(true);
        let export_btn = gtk::Button::with_label("Export");
        scheme_row.append(&scheme_lbl);
        scheme_row.append(&color_scheme_combo);
        scheme_row.append(&export_btn);
        color_box.append(&scheme_row);

        // Color palette section
        let pal_section_box = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        let pal_title_lbl = gtk::Label::new(None);
        pal_title_lbl.set_markup("<b>Color palette</b>");
        pal_title_lbl.add_css_class("heading");
        pal_title_lbl.set_valign(gtk::Align::Start);
        pal_section_box.append(&pal_title_lbl);

        let pal_grid = gtk::Grid::new();
        pal_grid.set_column_spacing(32);
        pal_grid.set_row_spacing(8);

        let bg_color_btn = create_color_button();
        let fg_color_btn = create_color_button();
        let mut palette_buttons: Vec<gtk::ColorDialogButton> = Vec::with_capacity(16);
        for _ in 0..16 {
            palette_buttons.push(create_color_button());
        }

        // Left column
        let bg_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        bg_box.append(&bg_color_btn);
        bg_box.append(&gtk::Label::new(Some("Background")));
        pal_grid.attach(&bg_box, 0, 0, 1, 1);

        let b0_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        b0_box.append(&palette_buttons[0]);
        b0_box.append(&palette_buttons[8]);
        b0_box.append(&gtk::Label::new(Some("Black")));
        pal_grid.attach(&b0_box, 0, 1, 1, 1);

        let b1_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        b1_box.append(&palette_buttons[1]);
        b1_box.append(&palette_buttons[9]);
        b1_box.append(&gtk::Label::new(Some("Red")));
        pal_grid.attach(&b1_box, 0, 2, 1, 1);

        let b2_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        b2_box.append(&palette_buttons[2]);
        b2_box.append(&palette_buttons[10]);
        b2_box.append(&gtk::Label::new(Some("Green")));
        pal_grid.attach(&b2_box, 0, 3, 1, 1);

        let b3_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        b3_box.append(&palette_buttons[3]);
        b3_box.append(&palette_buttons[11]);
        b3_box.append(&gtk::Label::new(Some("Orange")));
        pal_grid.attach(&b3_box, 0, 4, 1, 1);

        // Right column
        let fg_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        fg_box.append(&fg_color_btn);
        fg_box.append(&gtk::Label::new(Some("Foreground")));
        pal_grid.attach(&fg_box, 1, 0, 1, 1);

        let b4_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        b4_box.append(&palette_buttons[4]);
        b4_box.append(&palette_buttons[12]);
        b4_box.append(&gtk::Label::new(Some("Blue")));
        pal_grid.attach(&b4_box, 1, 1, 1, 1);

        let b5_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        b5_box.append(&palette_buttons[5]);
        b5_box.append(&palette_buttons[13]);
        b5_box.append(&gtk::Label::new(Some("Purple")));
        pal_grid.attach(&b5_box, 1, 2, 1, 1);

        let b6_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        b6_box.append(&palette_buttons[6]);
        b6_box.append(&palette_buttons[14]);
        b6_box.append(&gtk::Label::new(Some("Turquoise")));
        pal_grid.attach(&b6_box, 1, 3, 1, 1);

        let b7_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        b7_box.append(&palette_buttons[7]);
        b7_box.append(&palette_buttons[15]);
        b7_box.append(&gtk::Label::new(Some("Grey")));
        pal_grid.attach(&b7_box, 1, 4, 1, 1);

        pal_section_box.append(&pal_grid);
        color_box.append(&pal_section_box);

        // Options section
        let opt_title_lbl = gtk::Label::new(None);
        opt_title_lbl.set_markup("<b>Options</b>");
        opt_title_lbl.add_css_class("heading");
        opt_title_lbl.set_halign(gtk::Align::Start);
        color_box.append(&opt_title_lbl);

        let theme_colors_check = gtk::CheckButton::with_label("Use theme colors for foreground/background");
        let advanced_btn = gtk::MenuButton::new();
        let adv_btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        adv_btn_box.append(&gtk::Label::new(Some("Advanced")));
        adv_btn_box.append(&gtk::Image::from_icon_name("pan-down-symbolic"));
        advanced_btn.set_child(Some(&adv_btn_box));

        // Advanced Popover for Color Overrides
        let adv_popover = gtk::Popover::new();
        let adv_pop_vbox = gtk::Box::new(gtk::Orientation::Vertical, 8);
        adv_pop_vbox.set_margin_top(10);
        adv_pop_vbox.set_margin_bottom(10);
        adv_pop_vbox.set_margin_start(10);
        adv_pop_vbox.set_margin_end(10);

        let bold_override_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let bold_override_check = gtk::CheckButton::with_label("Override Bold Color");
        let bold_color_btn = create_color_button();
        bold_override_box.append(&bold_override_check);
        bold_override_box.append(&bold_color_btn);
        adv_pop_vbox.append(&bold_override_box);

        let cur_override_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let cursor_override_check = gtk::CheckButton::with_label("Override Cursor Colors");
        let cursor_bg_btn = create_color_button();
        let cursor_fg_btn = create_color_button();
        cur_override_box.append(&cursor_override_check);
        cur_override_box.append(&cursor_bg_btn);
        cur_override_box.append(&cursor_fg_btn);
        adv_pop_vbox.append(&cur_override_box);

        let hl_override_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let highlight_override_check = gtk::CheckButton::with_label("Override Highlight Colors");
        let hl_bg_btn = create_color_button();
        let hl_fg_btn = create_color_button();
        hl_override_box.append(&highlight_override_check);
        hl_override_box.append(&hl_bg_btn);
        hl_override_box.append(&hl_fg_btn);
        adv_pop_vbox.append(&hl_override_box);

        adv_popover.set_child(Some(&adv_pop_vbox));
        advanced_btn.set_popover(Some(&adv_popover));

        let opt_row1 = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        opt_row1.append(&theme_colors_check);
        opt_row1.append(&advanced_btn);
        color_box.append(&opt_row1);

        let bold_bright_check = gtk::CheckButton::with_label("Show bold text in bright colors");
        color_box.append(&bold_bright_check);

        // Sliders
        let slider_grid = gtk::Grid::new();
        slider_grid.set_column_spacing(12);
        slider_grid.set_row_spacing(8);

        let trans_lbl = gtk::Label::new(Some("Transparency"));
        trans_lbl.set_halign(gtk::Align::End);
        trans_lbl.set_xalign(1.0);
        let trans_scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
        trans_scale.set_draw_value(false);
        trans_scale.set_hexpand(true);
        slider_grid.attach(&trans_lbl, 0, 0, 1, 1);
        slider_grid.attach(&trans_scale, 1, 0, 1, 1);

        let dim_lbl = gtk::Label::new(Some("Unfocused dim"));
        dim_lbl.set_halign(gtk::Align::End);
        dim_lbl.set_xalign(1.0);
        let dim_scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
        dim_scale.set_draw_value(false);
        dim_scale.set_hexpand(true);
        slider_grid.attach(&dim_lbl, 0, 1, 1, 1);
        slider_grid.attach(&dim_scale, 1, 1, 1, 1);

        color_box.append(&slider_grid);

        let color_scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .propagate_natural_height(true)
            .hexpand(true)
            .vexpand(true)
            .child(&color_box)
            .build();
        notebook.append_page(&color_scrolled, Some(&gtk::Label::new(Some("Color"))));

        // -------------------------------------------------------------------------
        // 4. Scrolling Tab
        // -------------------------------------------------------------------------
        let scroll_box = gtk::Box::new(gtk::Orientation::Vertical, 14);
        scroll_box.set_margin_start(20);
        scroll_box.set_margin_end(20);
        scroll_box.set_margin_top(16);
        scroll_box.set_margin_bottom(16);

        let scrollbar_check = gtk::CheckButton::with_label("Show scrollbar");
        let scroll_out_check = gtk::CheckButton::with_label("Scroll on output");
        let scroll_key_check = gtk::CheckButton::with_label("Scroll on keystroke");

        let limit_scroll_box = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let limit_scroll_check = gtk::CheckButton::with_label("Limit scrollback to:");
        let scroll_lines_adj = gtk::Adjustment::new(8192.0, 100.0, 100000.0, 500.0, 1000.0, 0.0);
        let scroll_lines_spin = gtk::SpinButton::new(Some(&scroll_lines_adj), 1.0, 0);
        scroll_lines_spin.set_halign(gtk::Align::Start);
        scroll_lines_spin.set_size_request(140, -1);
        limit_scroll_box.append(&limit_scroll_check);
        limit_scroll_box.append(&scroll_lines_spin);

        scroll_box.append(&scrollbar_check);
        scroll_box.append(&scroll_out_check);
        scroll_box.append(&scroll_key_check);
        scroll_box.append(&limit_scroll_box);

        let scroll_scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .propagate_natural_height(true)
            .hexpand(true)
            .vexpand(true)
            .child(&scroll_box)
            .build();
        notebook.append_page(&scroll_scrolled, Some(&gtk::Label::new(Some("Scrolling"))));

        // -------------------------------------------------------------------------
        // 5. Compatibility Tab
        // -------------------------------------------------------------------------
        let compat_grid = gtk::Grid::new();
        compat_grid.set_column_spacing(16);
        compat_grid.set_row_spacing(12);
        compat_grid.set_margin_start(20);
        compat_grid.set_margin_end(20);
        compat_grid.set_margin_top(16);
        compat_grid.set_margin_bottom(16);

        let erase_names = [
            "Automatic",
            "Control-H",
            "ASCII DEL",
            "Escape sequence",
            "TTY",
        ];

        let bs_lbl = gtk::Label::new(Some("Backspace key generates"));
        bs_lbl.set_halign(gtk::Align::End);
        bs_lbl.set_xalign(1.0);
        let bs_model = gtk::StringList::new(&erase_names);
        let backspace_combo = gtk::DropDown::new(Some(bs_model), gtk::Expression::NONE);
        backspace_combo.set_halign(gtk::Align::Start);
        backspace_combo.set_size_request(240, -1);
        compat_grid.attach(&bs_lbl, 0, 0, 1, 1);
        compat_grid.attach(&backspace_combo, 1, 0, 1, 1);

        let del_lbl = gtk::Label::new(Some("Delete key generates"));
        del_lbl.set_halign(gtk::Align::End);
        del_lbl.set_xalign(1.0);
        let del_model = gtk::StringList::new(&erase_names);
        let delete_combo = gtk::DropDown::new(Some(del_model), gtk::Expression::NONE);
        delete_combo.set_halign(gtk::Align::Start);
        delete_combo.set_size_request(240, -1);
        compat_grid.attach(&del_lbl, 0, 1, 1, 1);
        compat_grid.attach(&delete_combo, 1, 1, 1, 1);

        let enc_lbl = gtk::Label::new(Some("Encoding"));
        enc_lbl.set_halign(gtk::Align::End);
        enc_lbl.set_xalign(1.0);
        let enc_names = ["UTF-8 Unicode", "ISO-8859-1", "Windows-1252", "US-ASCII"];
        let enc_model = gtk::StringList::new(&enc_names);
        let encoding_combo = gtk::DropDown::new(Some(enc_model), gtk::Expression::NONE);
        encoding_combo.set_halign(gtk::Align::Start);
        encoding_combo.set_size_request(240, -1);
        compat_grid.attach(&enc_lbl, 0, 2, 1, 1);
        compat_grid.attach(&encoding_combo, 1, 2, 1, 1);

        let cjk_lbl = gtk::Label::new(Some("Ambiguous-width characters"));
        cjk_lbl.set_halign(gtk::Align::End);
        cjk_lbl.set_xalign(1.0);
        let cjk_names = ["Narrow", "Wide"];
        let cjk_model = gtk::StringList::new(&cjk_names);
        let cjk_combo = gtk::DropDown::new(Some(cjk_model), gtk::Expression::NONE);
        cjk_combo.set_halign(gtk::Align::Start);
        cjk_combo.set_size_request(240, -1);
        compat_grid.attach(&cjk_lbl, 0, 3, 1, 1);
        compat_grid.attach(&cjk_combo, 1, 3, 1, 1);

        let osc52_check =
            gtk::CheckButton::with_label("Allow terminal applications to set clipboard (OSC 52)");
        osc52_check.set_tooltip_text(Some(
            "Intercept OSC 52 escape sequences to update desktop clipboard (applies to newly created panes)",
        ));
        compat_grid.attach(&osc52_check, 1, 4, 1, 1);

        let osc52_query_check =
            gtk::CheckButton::with_label("Allow terminal applications to read clipboard (OSC 52 query)");
        osc52_query_check.set_tooltip_text(Some(
            "Allow applications to query clipboard contents via OSC 52 (applies to newly created panes)",
        ));
        compat_grid.attach(&osc52_query_check, 1, 5, 1, 1);

        let compat_scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .propagate_natural_height(true)
            .hexpand(true)
            .vexpand(true)
            .child(&compat_grid)
            .build();
        notebook.append_page(&compat_scrolled, Some(&gtk::Label::new(Some("Compatibility"))));

        // -------------------------------------------------------------------------
        // 6. Badge Tab
        // -------------------------------------------------------------------------
        let badge_grid = gtk::Grid::new();
        badge_grid.set_column_spacing(16);
        badge_grid.set_row_spacing(12);
        badge_grid.set_margin_start(20);
        badge_grid.set_margin_end(20);
        badge_grid.set_margin_top(16);
        badge_grid.set_margin_bottom(16);

        let badge_lbl = gtk::Label::new(Some("Badge"));
        badge_lbl.set_halign(gtk::Align::End);
        badge_lbl.set_xalign(1.0);
        let badge_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        badge_box.add_css_class("linked");
        let badge_entry = gtk::Entry::new();
        badge_entry.set_hexpand(true);
        let badge_token_btn = create_scoped_token_menu_button(&badge_entry, TitleEditScope::Terminal);
        badge_box.append(&badge_entry);
        badge_box.append(&badge_token_btn);
        badge_grid.attach(&badge_lbl, 0, 0, 1, 1);
        badge_grid.attach(&badge_box, 1, 0, 1, 1);

        let badge_pos_lbl = gtk::Label::new(Some("Badge position"));
        badge_pos_lbl.set_halign(gtk::Align::End);
        badge_pos_lbl.set_xalign(1.0);
        let badge_pos_names = ["Northwest", "Northeast", "Southwest", "Southeast"];
        let badge_pos_model = gtk::StringList::new(&badge_pos_names);
        let badge_pos_combo = gtk::DropDown::new(Some(badge_pos_model), gtk::Expression::NONE);
        badge_pos_combo.set_halign(gtk::Align::Start);
        badge_pos_combo.set_size_request(240, -1);
        badge_grid.attach(&badge_pos_lbl, 0, 1, 1, 1);
        badge_grid.attach(&badge_pos_combo, 1, 1, 1, 1);

        let badge_font_lbl = gtk::Label::new(Some("Custom font"));
        badge_font_lbl.set_halign(gtk::Align::End);
        badge_font_lbl.set_xalign(1.0);
        let badge_font_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        badge_font_box.set_halign(gtk::Align::Start);
        let badge_font_check = gtk::CheckButton::new();
        let badge_font_btn = create_font_button();
        badge_font_box.append(&badge_font_check);
        badge_font_box.append(&badge_font_btn);
        badge_grid.attach(&badge_font_lbl, 0, 2, 1, 1);
        badge_grid.attach(&badge_font_box, 1, 2, 1, 1);

        let badge_scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .propagate_natural_height(true)
            .hexpand(true)
            .vexpand(true)
            .child(&badge_grid)
            .build();
        notebook.append_page(&badge_scrolled, Some(&gtk::Label::new(Some("Badge"))));

        // -------------------------------------------------------------------------
        // 7. Advanced Tab
        // -------------------------------------------------------------------------
        let adv_box = gtk::Box::new(gtk::Orientation::Vertical, 16);
        adv_box.set_margin_start(20);
        adv_box.set_margin_end(20);
        adv_box.set_margin_top(16);
        adv_box.set_margin_bottom(16);

        // Section 1: Notify New Activity
        let notify_title = gtk::Label::new(None);
        notify_title.set_markup("<b>Notify New Activity</b>");
        notify_title.add_css_class("heading");
        notify_title.set_halign(gtk::Align::Start);
        let notify_desc = gtk::Label::new(Some(
            "A notification can be raised when new activity occurs after a specified period of silence.",
        ));
        notify_desc.set_halign(gtk::Align::Start);
        notify_desc.set_wrap(true);
        notify_desc.set_opacity(0.7);

        let notify_grid = gtk::Grid::new();
        notify_grid.set_column_spacing(16);
        notify_grid.set_row_spacing(10);

        let silence_lbl = gtk::Label::new(Some("Enable by default"));
        silence_lbl.set_halign(gtk::Align::End);
        silence_lbl.set_xalign(1.0);
        let silence_check = gtk::CheckButton::new();
        notify_grid.attach(&silence_lbl, 0, 0, 1, 1);
        notify_grid.attach(&silence_check, 1, 0, 1, 1);

        let thresh_lbl = gtk::Label::new(Some("Threshold for continuous silence"));
        thresh_lbl.set_halign(gtk::Align::End);
        thresh_lbl.set_xalign(1.0);
        let thresh_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let silence_thresh_adj = gtk::Adjustment::new(0.0, 0.0, 3600.0, 1.0, 5.0, 0.0);
        let silence_thresh_spin = gtk::SpinButton::new(Some(&silence_thresh_adj), 1.0, 0);
        silence_thresh_spin.set_size_request(100, -1);
        thresh_box.append(&silence_thresh_spin);
        thresh_box.append(&gtk::Label::new(Some("(seconds)")));
        notify_grid.attach(&thresh_lbl, 0, 1, 1, 1);
        notify_grid.attach(&thresh_box, 1, 1, 1, 1);

        adv_box.append(&notify_title);
        adv_box.append(&notify_desc);
        adv_box.append(&notify_grid);

        // Section 2: Custom Links
        let links_title = gtk::Label::new(None);
        links_title.set_markup("<b>Custom Links</b>");
        links_title.add_css_class("heading");
        links_title.set_halign(gtk::Align::Start);

        let links_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let links_desc = gtk::Label::new(Some(
            "A list of user defined links that can be clicked on in the terminal based on regular expression definitions.",
        ));
        links_desc.set_halign(gtk::Align::Start);
        links_desc.set_wrap(true);
        links_desc.set_hexpand(true);
        links_desc.set_opacity(0.7);
        let custom_links_btn = gtk::Button::with_label("Edit");
        links_box.append(&links_desc);
        links_box.append(&custom_links_btn);

        adv_box.append(&links_title);
        adv_box.append(&links_box);

        // Section 3: Automatic Profile Switching
        let auto_title = gtk::Label::new(None);
        auto_title.set_markup("<b>Automatic Profile Switching</b>");
        auto_title.add_css_class("heading");
        auto_title.set_halign(gtk::Align::Start);
        let auto_desc = gtk::Label::new(Some(
            "Profiles are automatically selected based on the values entered here. Values are entered using a hostname:directory format. Either the hostname or directory can be omitted but the colon must be present. Entries with neither hostname or directory are not permitted.",
        ));
        auto_desc.set_halign(gtk::Align::Start);
        auto_desc.set_wrap(true);
        auto_desc.set_opacity(0.7);

        let auto_content_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let list_col_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
        list_col_box.set_hexpand(true);
        let match_header = gtk::Label::new(Some("Match"));
        match_header.set_halign(gtk::Align::Start);
        match_header.set_margin_start(4);
        match_header.add_css_class("dim-label");

        let rules_list_box = gtk::ListBox::new();
        rules_list_box.set_selection_mode(gtk::SelectionMode::Single);
        let rules_scrolled = gtk::ScrolledWindow::new();
        rules_scrolled.set_min_content_height(120);
        rules_scrolled.set_child(Some(&rules_list_box));
        let rules_frame = gtk::Frame::new(None);
        rules_frame.set_child(Some(&rules_scrolled));
        list_col_box.append(&match_header);
        list_col_box.append(&rules_frame);

        let rule_btn_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let add_rule_btn = gtk::Button::with_label("Add");
        let edit_rule_btn = gtk::Button::with_label("Edit");
        let del_rule_btn = gtk::Button::with_label("Delete");
        rule_btn_box.append(&add_rule_btn);
        rule_btn_box.append(&edit_rule_btn);
        rule_btn_box.append(&del_rule_btn);

        auto_content_box.append(&list_col_box);
        auto_content_box.append(&rule_btn_box);

        adv_box.append(&auto_title);
        adv_box.append(&auto_desc);
        adv_box.append(&auto_content_box);

        let adv_scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .propagate_natural_height(true)
            .hexpand(true)
            .vexpand(true)
            .child(&adv_box)
            .build();
        notebook.append_page(&adv_scrolled, Some(&gtk::Label::new(Some("Advanced"))));

        // Assemble Profile Page Container
        let profile_page_container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        profile_page_container.set_vexpand(true);
        profile_page_container.set_hexpand(true);
        profile_page_container.append(&profile_header_box);
        let header_sep = gtk::Separator::new(gtk::Orientation::Horizontal);
        profile_page_container.append(&header_sep);
        profile_page_container.append(&notebook);

        let profile_page_group = adw::PreferencesGroup::new();
        profile_page_group.set_vexpand(true);
        profile_page_group.set_hexpand(true);
        profile_page_group.add(&profile_page_container);
        profiles_page.add(&profile_page_group);
        window.add(&profiles_page);

        // -------------------------------------------------------------------------
        // State & Reactive Synchronization
        // -------------------------------------------------------------------------
        let is_populating = Rc::new(Cell::new(false));
        let selected_id = Rc::new(RefCell::new(
            current_config.borrow().default_profile_id.clone(),
        ));

        // Refresh rules list helper
        let refresh_rules_list = {
            let rules_list_box = rules_list_box.clone();
            let current_config = Rc::clone(&current_config);
            let selected_id = Rc::clone(&selected_id);
            Rc::new(move || {
                while let Some(child) = rules_list_box.first_child() {
                    rules_list_box.remove(&child);
                }
                let cfg = current_config.borrow();
                let curr_id = selected_id.borrow().clone();
                if let Some(prof) = cfg.get_profile(&curr_id) {
                    for rule in &prof.automatic_switch {
                        let text = format!("{}:{}", rule.hostname, rule.directory);
                        let row_lbl = gtk::Label::new(Some(&text));
                        row_lbl.set_halign(gtk::Align::Start);
                        row_lbl.set_margin_top(4);
                        row_lbl.set_margin_bottom(4);
                        row_lbl.set_margin_start(8);
                        rules_list_box.append(&row_lbl);
                    }
                }
            })
        };

        // Populate fields from Profile
        let populate_fields = {
            let is_populating = Rc::clone(&is_populating);
            let name_r = name_entry.clone();
            let title_r = title_entry.clone();
            let cols_r = cols_spin.clone();
            let rows_r = rows_spin.clone();
            let cell_w_r = cell_w_spin.clone();
            let cell_h_r = cell_h_spin.clone();
            let margin_r = margin_spin.clone();
            let blink_r = blink_combo.clone();
            let cfont_check_r = custom_font_check.clone();
            let font_btn_r = font_btn.clone();
            let word_r = word_entry.clone();
            let cshape_r = cursor_shape_combo.clone();
            let cblink_r = cursor_blink_combo.clone();
            let bell_r = bell_combo.clone();

            let login_r = login_shell_check.clone();
            let custom_sw_r = custom_cmd_check.clone();
            let custom_cmd_r = custom_cmd_entry.clone();
            let exit_r = exit_action_combo.clone();

            let color_r = color_scheme_combo.clone();
            let bg_btn_r = bg_color_btn.clone();
            let fg_btn_r = fg_color_btn.clone();
            let pal_btns_r = palette_buttons.clone();
            let theme_col_r = theme_colors_check.clone();
            let bold_bright_r = bold_bright_check.clone();
            let trans_scale_r = trans_scale.clone();
            let dim_scale_r = dim_scale.clone();

            let bold_override_r = bold_override_check.clone();
            let bold_color_r = bold_color_btn.clone();
            let cur_override_r = cursor_override_check.clone();
            let cur_bg_r = cursor_bg_btn.clone();
            let cur_fg_r = cursor_fg_btn.clone();
            let hl_override_r = highlight_override_check.clone();
            let hl_bg_r = hl_bg_btn.clone();
            let hl_fg_r = hl_fg_btn.clone();

            let scr_bar_r = scrollbar_check.clone();
            let scr_out_r = scroll_out_check.clone();
            let scr_key_r = scroll_key_check.clone();
            let scr_limit_r = limit_scroll_check.clone();
            let scr_lines_r = scroll_lines_spin.clone();

            let bs_r = backspace_combo.clone();
            let del_r = delete_combo.clone();
            let enc_r = encoding_combo.clone();
            let cjk_r = cjk_combo.clone();

            let badge_txt_r = badge_entry.clone();
            let badge_pos_r = badge_pos_combo.clone();
            let badge_font_check_r = badge_font_check.clone();
            let badge_font_btn_r = badge_font_btn.clone();

            let sil_r = silence_check.clone();
            let sil_th_r = silence_thresh_spin.clone();
            let copy_on_select_r = copy_on_select_check.clone();
            let osc52_r = osc52_check.clone();
            let osc52_query_r = osc52_query_check.clone();
            let refresh_rules = Rc::clone(&refresh_rules_list);

            Rc::new(move |p: &Profile| {
                is_populating.set(true);

                name_r.set_text(&p.name);
                title_r.set_text(&p.terminal_title);
                cols_r.set_value(p.default_size_columns as f64);
                rows_r.set_value(p.default_size_rows as f64);
                cell_w_r.set_value(p.cell_width_scale);
                cell_h_r.set_value(p.cell_height_scale);
                margin_r.set_value(p.draw_margin as f64);

                let blink_idx = match p.text_blink_mode {
                    TextBlinkModePreference::Never => 0,
                    TextBlinkModePreference::Focused => 1,
                    TextBlinkModePreference::Unfocused => 2,
                    TextBlinkModePreference::Always => 3,
                };
                blink_r.set_selected(blink_idx);

                cfont_check_r.set_active(!p.use_system_font);
                font_btn_r.set_sensitive(!p.use_system_font);
                let font_name = p.font.as_deref().unwrap_or("Monospace 10");
                let fdesc = gtk::pango::FontDescription::from_string(font_name);
                font_btn_r.set_font_desc(&fdesc);

                word_r.set_text(&p.select_by_word_chars);

                let shape_idx = match p.cursor_shape {
                    CursorShapePreference::Block => 0,
                    CursorShapePreference::IBeam => 1,
                    CursorShapePreference::Underline => 2,
                };
                cshape_r.set_selected(shape_idx);

                let cblink_idx = match p.cursor_blink {
                    CursorBlinkPreference::System => 0,
                    CursorBlinkPreference::On => 1,
                    CursorBlinkPreference::Off => 2,
                };
                cblink_r.set_selected(cblink_idx);

                let bell_idx = match p.terminal_bell {
                    TerminalBellPreference::None => 0,
                    TerminalBellPreference::Sound => 1,
                    TerminalBellPreference::Icon => 2,
                    TerminalBellPreference::IconSound => 3,
                };
                bell_r.set_selected(bell_idx);

                login_r.set_active(p.login_shell);
                custom_sw_r.set_active(p.use_custom_command);
                custom_cmd_r.set_text(&p.custom_command);
                custom_cmd_r.set_sensitive(p.use_custom_command);

                let exit_idx = match p.exit_action {
                    ExitActionPreference::Close => 0,
                    ExitActionPreference::Restart => 1,
                    ExitActionPreference::Hold => 2,
                };
                exit_r.set_selected(exit_idx);

                let scheme_idx = match p.color_scheme.name.as_str() {
                    "Tilix Light" => 1,
                    "Solarized Dark" => 2,
                    "Monokai" => 3,
                    "Tilix Dark" => 0,
                    _ => 4,
                };
                color_r.set_selected(scheme_idx);

                bg_btn_r.set_rgba(&rgb_to_rgba(&p.color_scheme.background));
                fg_btn_r.set_rgba(&rgb_to_rgba(&p.color_scheme.foreground));
                for (btn, col) in pal_btns_r.iter().zip(p.color_scheme.palette.iter()) {
                    btn.set_rgba(&rgb_to_rgba(col));
                }

                theme_col_r.set_active(p.use_theme_colors);
                bold_bright_r.set_active(p.bold_is_bright);
                trans_scale_r.set_value(p.background_transparency_percent as f64);
                dim_scale_r.set_value(p.dim_transparency_percent as f64);

                bold_override_r.set_active(p.bold_color_set);
                bold_color_r.set_sensitive(p.bold_color_set);
                if let Some(ref c) = p.bold_color {
                    bold_color_r.set_rgba(&rgb_to_rgba(c));
                }

                cur_override_r.set_active(p.cursor_colors_set);
                cur_bg_r.set_sensitive(p.cursor_colors_set);
                cur_fg_r.set_sensitive(p.cursor_colors_set);
                if let Some(ref c) = p.cursor_background_color {
                    cur_bg_r.set_rgba(&rgb_to_rgba(c));
                }
                if let Some(ref c) = p.cursor_foreground_color {
                    cur_fg_r.set_rgba(&rgb_to_rgba(c));
                }

                hl_override_r.set_active(p.highlight_colors_set);
                hl_bg_r.set_sensitive(p.highlight_colors_set);
                hl_fg_r.set_sensitive(p.highlight_colors_set);
                if let Some(ref c) = p.highlight_background_color {
                    hl_bg_r.set_rgba(&rgb_to_rgba(c));
                }
                if let Some(ref c) = p.highlight_foreground_color {
                    hl_fg_r.set_rgba(&rgb_to_rgba(c));
                }

                scr_bar_r.set_active(p.show_scrollbar);
                scr_out_r.set_active(p.scroll_on_output);
                scr_key_r.set_active(p.scroll_on_keystroke);
                scr_limit_r.set_active(!p.scrollback_unlimited);
                scr_lines_r.set_sensitive(!p.scrollback_unlimited);
                scr_lines_r.set_value(p.scrollback_lines.unwrap_or(8192) as f64);

                let bs_idx = match p.backspace_binding {
                    EraseBindingPreference::AsciiBackspace => 1,
                    EraseBindingPreference::AsciiDelete => 2,
                    EraseBindingPreference::DeleteSequence => 3,
                    EraseBindingPreference::Tty => 4,
                    _ => 0,
                };
                bs_r.set_selected(bs_idx);

                let del_idx = match p.delete_binding {
                    EraseBindingPreference::AsciiBackspace => 1,
                    EraseBindingPreference::AsciiDelete => 2,
                    EraseBindingPreference::DeleteSequence => 3,
                    EraseBindingPreference::Tty => 4,
                    _ => 0,
                };
                del_r.set_selected(del_idx);

                let enc_idx = match p.encoding.as_str() {
                    "ISO-8859-1" => 1,
                    "Windows-1252" => 2,
                    "US-ASCII" => 3,
                    _ => 0,
                };
                enc_r.set_selected(enc_idx);

                let cjk_idx = match p.cjk_utf8_ambiguous_width {
                    CjkWidthPreference::Wide => 1,
                    _ => 0,
                };
                cjk_r.set_selected(cjk_idx);

                badge_txt_r.set_text(&p.badge_text);
                let bpos_idx = match p.badge_position {
                    BadgePosition::Northwest => 0,
                    BadgePosition::Southwest => 2,
                    BadgePosition::Southeast => 3,
                    _ => 1,
                };
                badge_pos_r.set_selected(bpos_idx);

                badge_font_check_r.set_active(!p.badge_use_system_font);
                badge_font_btn_r.set_sensitive(!p.badge_use_system_font);
                let b_font_name = p.badge_font.as_deref().unwrap_or("Monospace 12");
                let b_desc = gtk::pango::FontDescription::from_string(b_font_name);
                badge_font_btn_r.set_font_desc(&b_desc);

                sil_r.set_active(p.notify_silence_enabled);
                sil_th_r.set_value(p.notify_silence_threshold as f64);

                copy_on_select_r.set_active(p.copy_on_select);
                osc52_r.set_active(p.enable_osc52);
                osc52_query_r.set_active(p.osc52_allow_query);

                refresh_rules();

                is_populating.set(false);
            })
        };

        // Refresh profile selector dropdown list
        let refresh_profiles_dropdown = {
            let config_rc = Rc::clone(&current_config);
            let profile_dropdown = profile_dropdown.clone();
            let selected_id = Rc::clone(&selected_id);
            let del_btn = del_btn.clone();
            let set_def_btn = set_def_btn.clone();
            let populate_fields = Rc::clone(&populate_fields);
            let is_populating = Rc::clone(&is_populating);

            Rc::new(move || {
                is_populating.set(true);
                let cfg = config_rc.borrow();
                let names: Vec<String> = cfg
                    .profiles
                    .iter()
                    .map(|p| {
                        if p.id == cfg.default_profile_id {
                            format!("{} (Default)", p.name)
                        } else {
                            p.name.clone()
                        }
                    })
                    .collect();
                let name_refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
                let str_list = gtk::StringList::new(&name_refs);
                profile_dropdown.set_model(Some(&str_list));

                del_btn.set_sensitive(cfg.profiles.len() > 1);

                let curr_id = selected_id.borrow().clone();
                let selected_idx = cfg
                    .profiles
                    .iter()
                    .position(|p| p.id == curr_id)
                    .unwrap_or(0);
                profile_dropdown.set_selected(selected_idx as u32);

                set_def_btn.set_sensitive(curr_id != cfg.default_profile_id);

                let prof_opt = cfg.profiles.get(selected_idx).cloned();
                drop(cfg);

                if let Some(prof) = prof_opt {
                    *selected_id.borrow_mut() = prof.id.clone();
                    populate_fields(&prof);
                }
                is_populating.set(false);
            })
        };

        refresh_profiles_dropdown();

        // Save current fields back to active profile
        let save_active_profile = {
            let config_rc = Rc::clone(&current_config);
            let selected_id = Rc::clone(&selected_id);
            let is_populating = Rc::clone(&is_populating);
            let on_change = Rc::clone(&on_change);

            let name_r = name_entry.clone();
            let title_r = title_entry.clone();
            let cols_r = cols_spin.clone();
            let rows_r = rows_spin.clone();
            let cell_w_r = cell_w_spin.clone();
            let cell_h_r = cell_h_spin.clone();
            let margin_r = margin_spin.clone();
            let blink_r = blink_combo.clone();
            let cfont_check_r = custom_font_check.clone();
            let font_btn_r = font_btn.clone();
            let word_r = word_entry.clone();
            let cshape_r = cursor_shape_combo.clone();
            let cblink_r = cursor_blink_combo.clone();
            let bell_r = bell_combo.clone();

            let login_r = login_shell_check.clone();
            let custom_sw_r = custom_cmd_check.clone();
            let custom_cmd_r = custom_cmd_entry.clone();
            let exit_r = exit_action_combo.clone();

            let theme_col_r = theme_colors_check.clone();
            let bold_bright_r = bold_bright_check.clone();
            let trans_scale_r = trans_scale.clone();
            let dim_scale_r = dim_scale.clone();

            let bold_override_r = bold_override_check.clone();
            let bold_color_r = bold_color_btn.clone();
            let cur_override_r = cursor_override_check.clone();
            let cur_bg_r = cursor_bg_btn.clone();
            let cur_fg_r = cursor_fg_btn.clone();
            let hl_override_r = highlight_override_check.clone();
            let hl_bg_r = hl_bg_btn.clone();
            let hl_fg_r = hl_fg_btn.clone();

            let scr_bar_r = scrollbar_check.clone();
            let scr_out_r = scroll_out_check.clone();
            let scr_key_r = scroll_key_check.clone();
            let scr_limit_r = limit_scroll_check.clone();
            let scr_lines_r = scroll_lines_spin.clone();

            let bs_r = backspace_combo.clone();
            let del_r = delete_combo.clone();
            let enc_r = encoding_combo.clone();
            let cjk_r = cjk_combo.clone();

            let badge_txt_r = badge_entry.clone();
            let badge_pos_r = badge_pos_combo.clone();
            let badge_font_check_r = badge_font_check.clone();
            let badge_font_btn_r = badge_font_btn.clone();

            let sil_r = silence_check.clone();
            let sil_th_r = silence_thresh_spin.clone();
            let copy_on_select_r = copy_on_select_check.clone();
            let osc52_r = osc52_check.clone();
            let osc52_query_r = osc52_query_check.clone();

            Rc::new(move || {
                if is_populating.get() {
                    return;
                }

                let id = selected_id.borrow().clone();
                let mut cfg = config_rc.borrow_mut();
                let Some(prof) = cfg.get_profile_mut(&id) else { return; };

                prof.name = name_r.text().to_string();
                prof.terminal_title = title_r.text().to_string();
                prof.default_size_columns = cols_r.value() as u32;
                prof.default_size_rows = rows_r.value() as u32;
                prof.cell_width_scale = cell_w_r.value();
                prof.cell_height_scale = cell_h_r.value();
                prof.draw_margin = margin_r.value() as u32;

                prof.text_blink_mode = match blink_r.selected() {
                    1 => TextBlinkModePreference::Focused,
                    2 => TextBlinkModePreference::Unfocused,
                    3 => TextBlinkModePreference::Always,
                    _ => TextBlinkModePreference::Never,
                };

                prof.use_system_font = !cfont_check_r.is_active();
                if let Some(desc) = font_btn_r.font_desc() {
                    prof.font = Some(desc.to_string());
                }
                prof.select_by_word_chars = word_r.text().to_string();

                prof.cursor_shape = match cshape_r.selected() {
                    1 => CursorShapePreference::IBeam,
                    2 => CursorShapePreference::Underline,
                    _ => CursorShapePreference::Block,
                };

                prof.cursor_blink = match cblink_r.selected() {
                    1 => CursorBlinkPreference::On,
                    2 => CursorBlinkPreference::Off,
                    _ => CursorBlinkPreference::System,
                };

                prof.terminal_bell = match bell_r.selected() {
                    0 => TerminalBellPreference::None,
                    2 => TerminalBellPreference::Icon,
                    3 => TerminalBellPreference::IconSound,
                    _ => TerminalBellPreference::Sound,
                };

                prof.login_shell = login_r.is_active();
                prof.use_custom_command = custom_sw_r.is_active();
                prof.custom_command = custom_cmd_r.text().to_string();

                prof.exit_action = match exit_r.selected() {
                    1 => ExitActionPreference::Restart,
                    2 => ExitActionPreference::Hold,
                    _ => ExitActionPreference::Close,
                };

                prof.use_theme_colors = theme_col_r.is_active();
                prof.bold_is_bright = bold_bright_r.is_active();
                prof.background_transparency_percent = trans_scale_r.value().round() as u32;
                prof.dim_transparency_percent = dim_scale_r.value().round() as u32;

                prof.bold_color_set = bold_override_r.is_active();
                prof.bold_color = Some(rgba_to_rgb(&bold_color_r.rgba()));

                prof.cursor_colors_set = cur_override_r.is_active();
                prof.cursor_background_color = Some(rgba_to_rgb(&cur_bg_r.rgba()));
                prof.cursor_foreground_color = Some(rgba_to_rgb(&cur_fg_r.rgba()));

                prof.highlight_colors_set = hl_override_r.is_active();
                prof.highlight_background_color = Some(rgba_to_rgb(&hl_bg_r.rgba()));
                prof.highlight_foreground_color = Some(rgba_to_rgb(&hl_fg_r.rgba()));

                prof.show_scrollbar = scr_bar_r.is_active();
                prof.scroll_on_output = scr_out_r.is_active();
                prof.scroll_on_keystroke = scr_key_r.is_active();
                prof.scrollback_unlimited = !scr_limit_r.is_active();
                prof.scrollback_lines = Some(scr_lines_r.value() as i64);

                prof.backspace_binding = match bs_r.selected() {
                    1 => EraseBindingPreference::AsciiBackspace,
                    2 => EraseBindingPreference::AsciiDelete,
                    3 => EraseBindingPreference::DeleteSequence,
                    4 => EraseBindingPreference::Tty,
                    _ => EraseBindingPreference::Auto,
                };

                prof.delete_binding = match del_r.selected() {
                    1 => EraseBindingPreference::AsciiBackspace,
                    2 => EraseBindingPreference::AsciiDelete,
                    3 => EraseBindingPreference::DeleteSequence,
                    4 => EraseBindingPreference::Tty,
                    _ => EraseBindingPreference::Auto,
                };

                prof.encoding = match enc_r.selected() {
                    1 => "ISO-8859-1".into(),
                    2 => "Windows-1252".into(),
                    3 => "US-ASCII".into(),
                    _ => "UTF-8".into(),
                };

                prof.cjk_utf8_ambiguous_width = match cjk_r.selected() {
                    1 => CjkWidthPreference::Wide,
                    _ => CjkWidthPreference::Narrow,
                };

                prof.badge_text = badge_txt_r.text().to_string();
                prof.badge_position = match badge_pos_r.selected() {
                    0 => BadgePosition::Northwest,
                    2 => BadgePosition::Southwest,
                    3 => BadgePosition::Southeast,
                    _ => BadgePosition::Northeast,
                };
                prof.badge_use_system_font = !badge_font_check_r.is_active();
                if let Some(desc) = badge_font_btn_r.font_desc() {
                    prof.badge_font = Some(desc.to_string());
                }

                prof.notify_silence_enabled = sil_r.is_active();
                prof.notify_silence_threshold = sil_th_r.value() as u32;

                prof.copy_on_select = copy_on_select_r.is_active();
                prof.enable_osc52 = osc52_r.is_active();
                prof.osc52_allow_query = osc52_query_r.is_active();

                let updated = prof.clone();
                if updated.id == cfg.default_profile_id {
                    cfg.default_profile = updated.clone();
                }

                let _ = cfg.save();
                crate::ui::window::apply_profile_to_all_sessions(&updated);
                on_change(&updated);
            })
        };

        // Wire profile selector dropdown selection
        {
            let config_rc = Rc::clone(&current_config);
            let selected_id = Rc::clone(&selected_id);
            let populate_fields = Rc::clone(&populate_fields);
            let is_populating = Rc::clone(&is_populating);
            let on_change = Rc::clone(&on_change);
            let set_def_btn = set_def_btn.clone();

            profile_dropdown.connect_selected_notify(move |combo| {
                if is_populating.get() {
                    return;
                }
                let idx = combo.selected() as usize;
                let cfg = config_rc.borrow();
                if let Some(prof) = cfg.profiles.get(idx) {
                    *selected_id.borrow_mut() = prof.id.clone();
                    populate_fields(prof);
                    set_def_btn.set_sensitive(prof.id != cfg.default_profile_id);
                    crate::ui::window::apply_profile_to_all_sessions(prof);
                    on_change(prof);
                }
            });
        }

        // Palette Live Edit -> Custom Scheme helper
        let update_palette_color = {
            let current_config = Rc::clone(&current_config);
            let selected_id = Rc::clone(&selected_id);
            let is_populating = Rc::clone(&is_populating);
            let color_scheme_combo = color_scheme_combo.clone();
            let on_change = Rc::clone(&on_change);

            Rc::new(move |update_fn: Box<dyn Fn(&mut ColorScheme)>| {
                if is_populating.get() {
                    return;
                }
                let id = selected_id.borrow().clone();
                let mut cfg = current_config.borrow_mut();
                let Some(prof) = cfg.get_profile_mut(&id) else { return; };

                update_fn(&mut prof.color_scheme);
                prof.color_scheme.name = "Custom".to_string();

                is_populating.set(true);
                color_scheme_combo.set_selected(4);
                is_populating.set(false);

                let updated = prof.clone();
                if updated.id == cfg.default_profile_id {
                    cfg.default_profile = updated.clone();
                }
                let _ = cfg.save();
                crate::ui::window::apply_profile_to_all_sessions(&updated);
                on_change(&updated);
            })
        };

        // Wire 18 palette color buttons
        {
            let upc = Rc::clone(&update_palette_color);
            bg_color_btn.connect_rgba_notify(move |b| {
                let col = rgba_to_rgb(&b.rgba());
                upc(Box::new(move |s| s.background = col.clone()));
            });
        }
        {
            let upc = Rc::clone(&update_palette_color);
            fg_color_btn.connect_rgba_notify(move |b| {
                let col = rgba_to_rgb(&b.rgba());
                upc(Box::new(move |s| s.foreground = col.clone()));
            });
        }
        for (i, btn) in palette_buttons.iter().enumerate() {
            let upc = Rc::clone(&update_palette_color);
            btn.connect_rgba_notify(move |b| {
                let col = rgba_to_rgb(&b.rgba());
                upc(Box::new(move |s| s.palette[i] = col.clone()));
            });
        }

        // Color scheme dropdown selection
        {
            let current_config = Rc::clone(&current_config);
            let selected_id = Rc::clone(&selected_id);
            let is_populating = Rc::clone(&is_populating);
            let on_change = Rc::clone(&on_change);
            let bg_btn = bg_color_btn.clone();
            let fg_btn = fg_color_btn.clone();
            let pal_btns = palette_buttons.clone();

            color_scheme_combo.connect_selected_notify(move |combo| {
                if is_populating.get() {
                    return;
                }
                let idx = combo.selected();
                if idx >= 4 {
                    return;
                }
                let scheme = match idx {
                    1 => ColorScheme::tilix_light(),
                    2 => ColorScheme::solarized_dark(),
                    3 => ColorScheme::monokai(),
                    _ => ColorScheme::tilix_dark(),
                };

                let id = selected_id.borrow().clone();
                let mut cfg = current_config.borrow_mut();
                let Some(prof) = cfg.get_profile_mut(&id) else { return; };
                prof.color_scheme = scheme.clone();

                is_populating.set(true);
                bg_btn.set_rgba(&rgb_to_rgba(&scheme.background));
                fg_btn.set_rgba(&rgb_to_rgba(&scheme.foreground));
                for (btn, col) in pal_btns.iter().zip(scheme.palette.iter()) {
                    btn.set_rgba(&rgb_to_rgba(col));
                }
                is_populating.set(false);

                let updated = prof.clone();
                if updated.id == cfg.default_profile_id {
                    cfg.default_profile = updated.clone();
                }
                let _ = cfg.save();
                crate::ui::window::apply_profile_to_all_sessions(&updated);
                on_change(&updated);
            });
        }

        // Export Color Scheme Button
        {
            let current_config = Rc::clone(&current_config);
            let selected_id = Rc::clone(&selected_id);
            export_btn.connect_clicked(move |_| {
                let cfg = current_config.borrow();
                let id = selected_id.borrow().clone();
                if let Some(prof) = cfg.get_profile(&id) {
                    if let Ok(json) = serde_json::to_string_pretty(&prof.color_scheme) {
                        let path = crate::model::AppConfig::config_dir().join("schemes");
                        let _ = std::fs::create_dir_all(&path);
                        let file_path = path.join(format!("{}.json", prof.color_scheme.name.to_lowercase().replace(' ', "-")));
                        let _ = std::fs::write(&file_path, json);
                    }
                }
            });
        }

        // Reset buttons in General tab
        {
            let cols = cols_spin.clone();
            let rows = rows_spin.clone();
            let s = Rc::clone(&save_active_profile);
            size_reset_btn.connect_clicked(move |_| {
                cols.set_value(80.0);
                rows.set_value(24.0);
                s();
            });
        }
        {
            let cell_w = cell_w_spin.clone();
            let cell_h = cell_h_spin.clone();
            let s = Rc::clone(&save_active_profile);
            spacing_reset_btn.connect_clicked(move |_| {
                cell_w.set_value(1.0);
                cell_h.set_value(1.0);
                s();
            });
        }

        // Sensitivity linkages
        {
            let cmd_entry = custom_cmd_entry.clone();
            let s = Rc::clone(&save_active_profile);
            custom_cmd_check.connect_toggled(move |cb| {
                cmd_entry.set_sensitive(cb.is_active());
                s();
            });
        }
        {
            let fbtn = font_btn.clone();
            let s = Rc::clone(&save_active_profile);
            custom_font_check.connect_toggled(move |cb| {
                fbtn.set_sensitive(cb.is_active());
                s();
            });
        }
        {
            let bfbtn = badge_font_btn.clone();
            let s = Rc::clone(&save_active_profile);
            badge_font_check.connect_toggled(move |cb| {
                bfbtn.set_sensitive(cb.is_active());
                s();
            });
        }
        {
            let lspin = scroll_lines_spin.clone();
            let s = Rc::clone(&save_active_profile);
            limit_scroll_check.connect_toggled(move |cb| {
                lspin.set_sensitive(cb.is_active());
                s();
            });
        }
        {
            let btn = bold_color_btn.clone();
            let s = Rc::clone(&save_active_profile);
            bold_override_check.connect_toggled(move |cb| {
                btn.set_sensitive(cb.is_active());
                s();
            });
        }
        {
            let bg = cursor_bg_btn.clone();
            let fg = cursor_fg_btn.clone();
            let s = Rc::clone(&save_active_profile);
            cursor_override_check.connect_toggled(move |cb| {
                bg.set_sensitive(cb.is_active());
                fg.set_sensitive(cb.is_active());
                s();
            });
        }
        {
            let bg = hl_bg_btn.clone();
            let fg = hl_fg_btn.clone();
            let s = Rc::clone(&save_active_profile);
            highlight_override_check.connect_toggled(move |cb| {
                bg.set_sensitive(cb.is_active());
                fg.set_sensitive(cb.is_active());
                s();
            });
        }

        // Automatic Profile Switching actions
        {
            let win_weak = window.downgrade();
            let current_config = Rc::clone(&current_config);
            let selected_id = Rc::clone(&selected_id);
            let refresh_rules = Rc::clone(&refresh_rules_list);
            let on_change = Rc::clone(&on_change);

            add_rule_btn.connect_clicked(move |_| {
                let Some(win) = win_weak.upgrade() else { return; };
                let cfg_rc = Rc::clone(&current_config);
                let sel_rc = Rc::clone(&selected_id);
                let rr = Rc::clone(&refresh_rules);
                let oc = Rc::clone(&on_change);
                let curr_id = sel_rc.borrow().clone();

                show_auto_switch_rule_dialog(&win, None, curr_id.clone(), move |new_rule| {
                    let mut cfg = cfg_rc.borrow_mut();
                    if let Some(prof) = cfg.get_profile_mut(&curr_id) {
                        prof.automatic_switch.push(new_rule);
                        let updated = prof.clone();
                        let _ = cfg.save();
                        rr();
                        crate::ui::window::apply_profile_to_all_sessions(&updated);
                        oc(&updated);
                    }
                });
            });
        }
        {
            let win_weak = window.downgrade();
            let current_config = Rc::clone(&current_config);
            let selected_id = Rc::clone(&selected_id);
            let rules_list = rules_list_box.clone();
            let refresh_rules = Rc::clone(&refresh_rules_list);
            let on_change = Rc::clone(&on_change);

            edit_rule_btn.connect_clicked(move |_| {
                let Some(row) = rules_list.selected_row() else { return; };
                let idx = row.index() as usize;
                let Some(win) = win_weak.upgrade() else { return; };

                let cfg_rc = Rc::clone(&current_config);
                let sel_rc = Rc::clone(&selected_id);
                let rr = Rc::clone(&refresh_rules);
                let oc = Rc::clone(&on_change);
                let curr_id = sel_rc.borrow().clone();

                let rule_opt = cfg_rc
                    .borrow()
                    .get_profile(&curr_id)
                    .and_then(|p| p.automatic_switch.get(idx).cloned());

                if let Some(existing) = rule_opt {
                    show_auto_switch_rule_dialog(&win, Some(&existing), curr_id.clone(), move |edited| {
                        let mut cfg = cfg_rc.borrow_mut();
                        if let Some(prof) = cfg.get_profile_mut(&curr_id) {
                            if idx < prof.automatic_switch.len() {
                                prof.automatic_switch[idx] = edited;
                                let updated = prof.clone();
                                let _ = cfg.save();
                                rr();
                                crate::ui::window::apply_profile_to_all_sessions(&updated);
                                oc(&updated);
                            }
                        }
                    });
                }
            });
        }
        {
            let current_config = Rc::clone(&current_config);
            let selected_id = Rc::clone(&selected_id);
            let rules_list = rules_list_box.clone();
            let refresh_rules = Rc::clone(&refresh_rules_list);
            let on_change = Rc::clone(&on_change);

            del_rule_btn.connect_clicked(move |_| {
                let Some(row) = rules_list.selected_row() else { return; };
                let idx = row.index() as usize;
                let curr_id = selected_id.borrow().clone();
                let mut cfg = current_config.borrow_mut();
                if let Some(prof) = cfg.get_profile_mut(&curr_id) {
                    if idx < prof.automatic_switch.len() {
                        prof.automatic_switch.remove(idx);
                        let updated = prof.clone();
                        let _ = cfg.save();
                        refresh_rules();
                        crate::ui::window::apply_profile_to_all_sessions(&updated);
                        on_change(&updated);
                    }
                }
            });
        }

        // Custom Links Button Dialog
        {
            let win_weak = window.downgrade();
            let current_config = Rc::clone(&current_config);
            let selected_id = Rc::clone(&selected_id);
            let on_change = Rc::clone(&on_change);

            custom_links_btn.connect_clicked(move |_| {
                let Some(win) = win_weak.upgrade() else { return; };
                let curr_id = selected_id.borrow().clone();
                let links = current_config
                    .borrow()
                    .get_profile(&curr_id)
                    .map(|p| p.custom_hyperlinks.clone())
                    .unwrap_or_default();

                let cfg_rc = Rc::clone(&current_config);
                let oc = Rc::clone(&on_change);
                show_custom_links_dialog(&win, links, move |new_links| {
                    let mut cfg = cfg_rc.borrow_mut();
                    if let Some(prof) = cfg.get_profile_mut(&curr_id) {
                        prof.custom_hyperlinks = new_links;
                        let updated = prof.clone();
                        let _ = cfg.save();
                        crate::ui::window::apply_profile_to_all_sessions(&updated);
                        oc(&updated);
                    }
                });
            });
        }

        // Connect changes on all widgets to save_active_profile
        macro_rules! connect_sync {
            ($widget:expr, notify_active) => {{
                let s = Rc::clone(&save_active_profile);
                $widget.connect_active_notify(move |_| s());
            }};
            ($widget:expr, notify_selected) => {{
                let s = Rc::clone(&save_active_profile);
                $widget.connect_selected_notify(move |_| s());
            }};
            ($widget:expr, changed_entry) => {{
                let s = Rc::clone(&save_active_profile);
                $widget.connect_changed(move |_| s());
            }};
            ($widget:expr, value_changed) => {{
                let s = Rc::clone(&save_active_profile);
                $widget.connect_value_changed(move |_| s());
            }};
            ($widget:expr, notify_rgba) => {{
                let s = Rc::clone(&save_active_profile);
                $widget.connect_rgba_notify(move |_| s());
            }};
            ($widget:expr, notify_font) => {{
                let s = Rc::clone(&save_active_profile);
                $widget.connect_font_desc_notify(move |_| s());
            }};
        }

        connect_sync!(name_entry, changed_entry);
        connect_sync!(title_entry, changed_entry);
        connect_sync!(cols_spin, value_changed);
        connect_sync!(rows_spin, value_changed);
        connect_sync!(cell_w_spin, value_changed);
        connect_sync!(cell_h_spin, value_changed);
        connect_sync!(margin_spin, value_changed);
        connect_sync!(blink_combo, notify_selected);
        connect_sync!(font_btn, notify_font);
        connect_sync!(word_entry, changed_entry);
        connect_sync!(cursor_shape_combo, notify_selected);
        connect_sync!(cursor_blink_combo, notify_selected);
        connect_sync!(bell_combo, notify_selected);

        connect_sync!(login_shell_check, notify_active);
        connect_sync!(custom_cmd_entry, changed_entry);
        connect_sync!(exit_action_combo, notify_selected);

        connect_sync!(theme_colors_check, notify_active);
        connect_sync!(bold_bright_check, notify_active);
        connect_sync!(trans_scale, value_changed);
        connect_sync!(dim_scale, value_changed);
        connect_sync!(bold_color_btn, notify_rgba);
        connect_sync!(cursor_bg_btn, notify_rgba);
        connect_sync!(cursor_fg_btn, notify_rgba);
        connect_sync!(hl_bg_btn, notify_rgba);
        connect_sync!(hl_fg_btn, notify_rgba);

        connect_sync!(scrollbar_check, notify_active);
        connect_sync!(scroll_out_check, notify_active);
        connect_sync!(scroll_key_check, notify_active);
        connect_sync!(scroll_lines_spin, value_changed);

        connect_sync!(backspace_combo, notify_selected);
        connect_sync!(delete_combo, notify_selected);
        connect_sync!(encoding_combo, notify_selected);
        connect_sync!(cjk_combo, notify_selected);

        connect_sync!(badge_entry, changed_entry);
        connect_sync!(badge_pos_combo, notify_selected);
        connect_sync!(badge_font_btn, notify_font);

        connect_sync!(silence_check, notify_active);
        connect_sync!(silence_thresh_spin, value_changed);

        connect_sync!(copy_on_select_check, notify_active);
        connect_sync!(osc52_check, notify_active);
        connect_sync!(osc52_query_check, notify_active);

        // Action button callbacks: Add Profile
        {
            let config_rc = Rc::clone(&current_config);
            let selected_id = Rc::clone(&selected_id);
            let refresh = Rc::clone(&refresh_profiles_dropdown);
            add_btn.connect_clicked(move |_| {
                let mut new_prof = Profile::default();
                let gen_id = format!("profile-{}", glib::uuid_string_random());
                new_prof.id = gen_id.clone();
                new_prof.name = "New Profile".into();
                let _ = config_rc.borrow_mut().add_profile(new_prof);
                let _ = config_rc.borrow().save();
                *selected_id.borrow_mut() = gen_id;
                refresh();
            });
        }

        // Action button callbacks: Duplicate Profile
        {
            let config_rc = Rc::clone(&current_config);
            let selected_id = Rc::clone(&selected_id);
            let refresh = Rc::clone(&refresh_profiles_dropdown);
            dup_btn.connect_clicked(move |_| {
                let curr_id = selected_id.borrow().clone();
                let res = config_rc.borrow_mut().duplicate_profile(&curr_id);
                if let Ok(cloned) = res {
                    let _ = config_rc.borrow().save();
                    *selected_id.borrow_mut() = cloned.id;
                    refresh();
                }
            });
        }

        // Action button callbacks: Delete Profile
        {
            let config_rc = Rc::clone(&current_config);
            let selected_id = Rc::clone(&selected_id);
            let refresh = Rc::clone(&refresh_profiles_dropdown);
            del_btn.connect_clicked(move |_| {
                let curr_id = selected_id.borrow().clone();
                let res = config_rc.borrow_mut().delete_profile(&curr_id);
                if res.is_ok() {
                    let _ = config_rc.borrow().save();
                    let next_id = config_rc.borrow().profiles[0].id.clone();
                    *selected_id.borrow_mut() = next_id;
                    refresh();
                }
            });
        }

        // Action button callbacks: Set as Default
        {
            let config_rc = Rc::clone(&current_config);
            let selected_id = Rc::clone(&selected_id);
            let refresh = Rc::clone(&refresh_profiles_dropdown);
            set_def_btn.connect_clicked(move |_| {
                let curr_id = selected_id.borrow().clone();
                let res = config_rc.borrow_mut().set_default_profile(&curr_id);
                if res.is_ok() {
                    let _ = config_rc.borrow().save();
                    refresh();
                }
            });
        }

        // =========================================================================
        // APPEARANCE PAGE (Window & Title)
        // =========================================================================
        let appearance_page = adw::PreferencesPage::new();
        appearance_page.set_title("Appearance");
        appearance_page.set_icon_name(Some("preferences-desktop-appearance-symbolic"));

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

        let compact_mode_row = adw::SwitchRow::new();
        compact_mode_row.set_title("Compact Mode");
        compact_mode_row.set_subtitle("Reduce padding and titlebar heights for maximum terminal display area");
        compact_mode_row.set_active(current_config.borrow().compact_mode);
        window_group.add(&compact_mode_row);

        appearance_page.add(&window_group);

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

        let session_name_row = adw::ActionRow::new();
        session_name_row.set_title("Default session name");
        session_name_row.set_subtitle("Title template used for session tabs");

        let session_name_entry = gtk::Entry::new();
        session_name_entry.set_text(&current_config.borrow().default_session_name);
        session_name_entry.set_valign(gtk::Align::Center);
        session_name_entry.set_hexpand(true);

        let session_token_btn = create_scoped_token_menu_button(&session_name_entry, TitleEditScope::Session);
        session_token_btn.set_valign(gtk::Align::Center);

        session_name_row.add_suffix(&session_name_entry);
        session_name_row.add_suffix(&session_token_btn);
        session_name_row.set_activatable_widget(Some(&session_name_entry));
        title_group.add(&session_name_row);

        let app_title_row = adw::ActionRow::new();
        app_title_row.set_title("Application title");
        app_title_row.set_subtitle("Title template used for the main window header");

        let app_title_entry = gtk::Entry::new();
        app_title_entry.set_text(&current_config.borrow().app_title);
        app_title_entry.set_valign(gtk::Align::Center);
        app_title_entry.set_hexpand(true);

        let app_token_btn = create_scoped_token_menu_button(&app_title_entry, TitleEditScope::Window);
        app_token_btn.set_valign(gtk::Align::Center);

        app_title_row.add_suffix(&app_title_entry);
        app_title_row.add_suffix(&app_token_btn);
        app_title_row.set_activatable_widget(Some(&app_title_entry));
        title_group.add(&app_title_row);

        appearance_page.add(&title_group);
        window.add(&appearance_page);

        // Sync Appearance changes
        {
            let config_rc = Rc::clone(&current_config);
            let w_style = window_style_row.clone();
            let w_handle = wide_handle_row.clone();
            let t_bar = show_tab_bar_row.clone();
            let c_mode = compact_mode_row.clone();
            let t_style = title_style_row.clone();
            let t_single = title_show_single_row.clone();

            let save_app = Rc::new(move || {
                let window_style = match w_style.selected() {
                    1 => WindowStyle::HideToolbar,
                    _ => WindowStyle::Normal,
                };
                let use_wide_handle = w_handle.is_active();
                let show_tab_bar = t_bar.is_active();
                let compact_mode = c_mode.is_active();
                let pane_title_style = match t_style.selected() {
                    1 => PaneTitleStyle::None,
                    _ => PaneTitleStyle::Normal,
                };
                let pane_title_show_when_single = t_single.is_active();

                let mut cfg = config_rc.borrow_mut();
                cfg.window_style = window_style;
                cfg.use_wide_handle = use_wide_handle;
                cfg.show_tab_bar = show_tab_bar;
                cfg.compact_mode = compact_mode;
                cfg.pane_title_style = pane_title_style;
                cfg.pane_title_show_when_single = pane_title_show_when_single;

                let _ = cfg.save();
                crate::ui::window::apply_window_style_to_all_windows(cfg.window_style);
                crate::ui::window::apply_wide_handle_to_all_sessions(cfg.use_wide_handle);
                crate::ui::window::apply_show_tab_bar_to_all_windows(cfg.show_tab_bar);
                crate::ui::window::apply_compact_mode_to_all_windows(cfg.compact_mode);
                crate::ui::window::apply_pane_title_settings_to_all_sessions(
                    cfg.pane_title_style,
                    cfg.pane_title_show_when_single,
                );
            });

            let s1 = Rc::clone(&save_app);
            window_style_row.connect_selected_notify(move |_| s1());
            let s2 = Rc::clone(&save_app);
            wide_handle_row.connect_active_notify(move |_| s2());
            let s3 = Rc::clone(&save_app);
            show_tab_bar_row.connect_active_notify(move |_| s3());
            let s4 = Rc::clone(&save_app);
            title_style_row.connect_selected_notify(move |_| s4());
            let s5 = Rc::clone(&save_app);
            title_show_single_row.connect_active_notify(move |_| s5());
            let s6 = Rc::clone(&save_app);
            compact_mode_row.connect_active_notify(move |_| s6());

            let config_rc_titles = Rc::clone(&current_config);
            let s_entry = session_name_entry.clone();
            let a_entry = app_title_entry.clone();
            let save_titles = Rc::new(move || {
                let default_session_name = s_entry.text().to_string();
                let app_title = a_entry.text().to_string();

                let mut cfg = config_rc_titles.borrow_mut();
                cfg.default_session_name = default_session_name;
                cfg.app_title = app_title;

                let _ = cfg.save();
                drop(cfg);
                crate::ui::window::apply_title_settings_to_all_windows();
            });

            let st1 = Rc::clone(&save_titles);
            session_name_entry.connect_changed(move |_| st1());
            let st2 = Rc::clone(&save_titles);
            app_title_entry.connect_changed(move |_| st2());
        }

        // =========================================================================
        // BEHAVIOR PAGE (Quake & Notifications)
        // =========================================================================
        let behavior_page = adw::PreferencesPage::new();
        behavior_page.set_title("Behavior");
        behavior_page.set_icon_name(Some("preferences-system-symbolic"));

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

        // Sync Behavior changes
        {
            let config_rc = Rc::clone(&current_config);
            let q_height = quake_height_row.clone();
            let q_unfocus = quake_unfocus_row.clone();
            let n_enabled = notif_enabled_row.clone();
            let n_bell = notif_bell_row.clone();
            let n_exit = notif_exit_row.clone();

            let save_beh = Rc::new(move || {
                let mut cfg = config_rc.borrow_mut();
                cfg.quake_height_percent = q_height.value() as u32;
                cfg.quake_hide_on_unfocus = q_unfocus.is_active();
                cfg.notifications_enabled = n_enabled.is_active();
                cfg.bell_notifications = n_bell.is_active();
                cfg.process_exit_notifications = n_exit.is_active();
                let _ = cfg.save();
            });

            let s1 = Rc::clone(&save_beh);
            quake_height_row.connect_changed(move |_| s1());
            let s2 = Rc::clone(&save_beh);
            quake_unfocus_row.connect_active_notify(move |_| s2());
            let s3 = Rc::clone(&save_beh);
            notif_enabled_row.connect_active_notify(move |_| s3());
            let s4 = Rc::clone(&save_beh);
            notif_bell_row.connect_active_notify(move |_| s4());
            let s5 = Rc::clone(&save_beh);
            notif_exit_row.connect_active_notify(move |_| s5());
        }

        // =========================================================================
        // SHORTCUTS PAGE
        // =========================================================================
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
            ActionCategory::Clipboard,
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

                let mut mods = state
                    & (gtk::gdk::ModifierType::CONTROL_MASK
                        | gtk::gdk::ModifierType::SHIFT_MASK
                        | gtk::gdk::ModifierType::ALT_MASK
                        | gtk::gdk::ModifierType::SUPER_MASK);

                // If the key produces an ASCII punctuation character (e.g. '+', '^', '<', '>', '?'),
                // the symbol itself inherently represents the shifted character.
                // Strip redundant Shift modifier so it displays cleanly as Ctrl++ / Ctrl+^ / Ctrl+<
                // while strictly preserving Shift for letters (e.g. Ctrl+Shift+V) and function keys.
                if keyval.to_unicode().map_or(false, |c| c.is_ascii_punctuation()) {
                    mods.remove(gtk::gdk::ModifierType::SHIFT_MASK);
                }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preferences_window_initialization_headless() {
        crate::ui::window::run_gtk_test(|| {
            let changed_profile = Rc::new(RefCell::new(None));
            let cp = Rc::clone(&changed_profile);
            let pref_win = TilixPreferencesWindow::new(None::<&gtk::Window>, move |p| {
                *cp.borrow_mut() = Some(p.clone());
            });

            assert_eq!(pref_win.window().title().as_deref(), Some("Preferences"));
        });
    }
}
