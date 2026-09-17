use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;

use crate::model::config::{AppConfig, PaneTitleStyle, WindowStyle};
use crate::model::keybindings::{ActionCategory, ActionShortcutDef, ACTION_CATALOG};
use crate::model::profile::{
    BadgePosition, CjkWidthPreference, CursorBlinkPreference, CursorShapePreference,
    EraseBindingPreference, ExitActionPreference, Profile, TerminalBellPreference,
    TextBlinkModePreference,
};
use crate::model::theme::{ColorScheme, RgbColor};

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
        window.set_default_size(750, 680);

        let current_config = Rc::new(RefCell::new(AppConfig::load()));
        let on_change = Rc::new(on_profile_changed);

        // =========================================================================
        // PROFILES PAGE
        // =========================================================================
        let profiles_page = adw::PreferencesPage::new();
        profiles_page.set_title("Profiles");
        profiles_page.set_icon_name(Some("org.gnome.Settings-symbolic"));

        let mgmt_group = adw::PreferencesGroup::new();
        mgmt_group.set_title("Profile Selection and Management");

        // Profile selector combo
        let profile_combo = adw::ComboRow::new();
        profile_combo.set_title("Selected Profile");

        // Action buttons
        let actions_row = adw::ActionRow::new();
        actions_row.set_title("Profile Actions");
        let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        btn_box.set_valign(gtk::Align::Center);

        let add_btn = gtk::Button::with_label("New");
        add_btn.set_tooltip_text(Some("Create a new profile"));
        add_btn.add_css_class("flat");

        let dup_btn = gtk::Button::with_label("Duplicate");
        dup_btn.set_tooltip_text(Some("Duplicate selected profile"));
        dup_btn.add_css_class("flat");

        let del_btn = gtk::Button::with_label("Delete");
        del_btn.set_tooltip_text(Some("Delete selected profile"));
        del_btn.add_css_class("flat");
        del_btn.add_css_class("destructive-action");

        let set_def_btn = gtk::Button::with_label("Set as Default");
        set_def_btn.set_tooltip_text(Some("Make this profile the default"));
        set_def_btn.add_css_class("flat");

        btn_box.append(&add_btn);
        btn_box.append(&dup_btn);
        btn_box.append(&del_btn);
        btn_box.append(&set_def_btn);
        actions_row.add_suffix(&btn_box);

        // Section tab combo
        let section_names = [
            "General",
            "Command",
            "Color",
            "Scrolling",
            "Compatibility",
            "Badge",
            "Advanced",
        ];
        let section_model = gtk::StringList::new(&section_names);
        let section_combo = adw::ComboRow::new();
        section_combo.set_title("Settings Section");
        section_combo.set_model(Some(&section_model));
        section_combo.set_selected(0);

        mgmt_group.add(&profile_combo);
        mgmt_group.add(&actions_row);
        mgmt_group.add(&section_combo);
        profiles_page.add(&mgmt_group);

        // -------------------------------------------------------------------------
        // 1. General Tab
        // -------------------------------------------------------------------------
        let general_group = adw::PreferencesGroup::new();
        general_group.set_title("General Settings");

        let prof_name_row = adw::EntryRow::new();
        prof_name_row.set_title("Profile Name");

        let prof_title_row = adw::EntryRow::new();
        prof_title_row.set_title("Terminal Title Format");

        let cols_adj = gtk::Adjustment::new(80.0, 20.0, 500.0, 1.0, 10.0, 0.0);
        let cols_row = adw::SpinRow::new(Some(&cols_adj), 1.0, 0);
        cols_row.set_title("Initial Columns");

        let rows_adj = gtk::Adjustment::new(24.0, 5.0, 200.0, 1.0, 5.0, 0.0);
        let rows_row = adw::SpinRow::new(Some(&rows_adj), 1.0, 0);
        rows_row.set_title("Initial Rows");

        let cell_w_adj = gtk::Adjustment::new(1.0, 1.0, 2.0, 0.05, 0.1, 0.0);
        let cell_w_row = adw::SpinRow::new(Some(&cell_w_adj), 0.05, 2);
        cell_w_row.set_title("Cell Width Scale");

        let cell_h_adj = gtk::Adjustment::new(1.0, 1.0, 2.0, 0.05, 0.1, 0.0);
        let cell_h_row = adw::SpinRow::new(Some(&cell_h_adj), 0.05, 2);
        cell_h_row.set_title("Cell Height Scale");

        let margin_adj = gtk::Adjustment::new(0.0, 0.0, 500.0, 1.0, 10.0, 0.0);
        let margin_row = adw::SpinRow::new(Some(&margin_adj), 1.0, 0);
        margin_row.set_title("Margin Column (0 = Disabled)");

        let blink_names = ["Never", "Focused", "Unfocused", "Always"];
        let blink_mode_model = gtk::StringList::new(&blink_names);
        let blink_mode_row = adw::ComboRow::new();
        blink_mode_row.set_title("Text Blink Mode");
        blink_mode_row.set_model(Some(&blink_mode_model));

        let allow_bold_row = adw::SwitchRow::new();
        allow_bold_row.set_title("Allow Bold Text");

        let rewrap_row = adw::SwitchRow::new();
        rewrap_row.set_title("Rewrap on Resize");

        let system_font_row = adw::SwitchRow::new();
        system_font_row.set_title("Use System Monospace Font");

        let font_row = adw::EntryRow::new();
        font_row.set_title("Custom Font");

        let word_chars_row = adw::EntryRow::new();
        word_chars_row.set_title("Word Selection Characters");

        let shape_names = ["Block", "I-Beam", "Underline"];
        let shape_model = gtk::StringList::new(&shape_names);
        let cursor_shape_row = adw::ComboRow::new();
        cursor_shape_row.set_title("Cursor Shape");
        cursor_shape_row.set_model(Some(&shape_model));

        let cblink_names = ["System", "On", "Off"];
        let cblink_model = gtk::StringList::new(&cblink_names);
        let cursor_blink_row = adw::ComboRow::new();
        cursor_blink_row.set_title("Cursor Blink");
        cursor_blink_row.set_model(Some(&cblink_model));

        let bell_names = ["None", "Sound", "Icon", "Icon and Sound"];
        let bell_model = gtk::StringList::new(&bell_names);
        let bell_row = adw::ComboRow::new();
        bell_row.set_title("Terminal Bell");
        bell_row.set_model(Some(&bell_model));

        general_group.add(&prof_name_row);
        general_group.add(&prof_title_row);
        general_group.add(&cols_row);
        general_group.add(&rows_row);
        general_group.add(&cell_w_row);
        general_group.add(&cell_h_row);
        general_group.add(&margin_row);
        general_group.add(&blink_mode_row);
        general_group.add(&allow_bold_row);
        general_group.add(&rewrap_row);
        general_group.add(&system_font_row);
        general_group.add(&font_row);
        general_group.add(&word_chars_row);
        general_group.add(&cursor_shape_row);
        general_group.add(&cursor_blink_row);
        general_group.add(&bell_row);
        profiles_page.add(&general_group);

        // -------------------------------------------------------------------------
        // 2. Command Tab
        // -------------------------------------------------------------------------
        let command_group = adw::PreferencesGroup::new();
        command_group.set_title("Command and Execution");

        let login_shell_row = adw::SwitchRow::new();
        login_shell_row.set_title("Run Command as Login Shell");

        let use_custom_cmd_row = adw::SwitchRow::new();
        use_custom_cmd_row.set_title("Run a Custom Command Instead of Shell");

        let custom_cmd_row = adw::EntryRow::new();
        custom_cmd_row.set_title("Custom Command");

        let exit_action_names = ["Close Terminal", "Restart Process", "Hold Terminal Open"];
        let exit_action_model = gtk::StringList::new(&exit_action_names);
        let exit_action_row = adw::ComboRow::new();
        exit_action_row.set_title("When Command Exits");
        exit_action_row.set_model(Some(&exit_action_model));

        command_group.add(&login_shell_row);
        command_group.add(&use_custom_cmd_row);
        command_group.add(&custom_cmd_row);
        command_group.add(&exit_action_row);
        profiles_page.add(&command_group);

        // -------------------------------------------------------------------------
        // 3. Color Tab
        // -------------------------------------------------------------------------
        let colors_group = adw::PreferencesGroup::new();
        colors_group.set_title("Color Scheme and Overrides");

        let color_names = ["Tilix Dark", "Tilix Light", "Solarized Dark", "Monokai"];
        let color_model = gtk::StringList::new(&color_names);
        let color_preset_row = adw::ComboRow::new();
        color_preset_row.set_title("Preset Color Scheme");
        color_preset_row.set_model(Some(&color_model));

        let use_theme_colors_row = adw::SwitchRow::new();
        use_theme_colors_row.set_title("Use System Theme Colors");

        let bg_trans_adj = gtk::Adjustment::new(0.0, 0.0, 100.0, 1.0, 5.0, 0.0);
        let bg_trans_row = adw::SpinRow::new(Some(&bg_trans_adj), 1.0, 0);
        bg_trans_row.set_title("Background Transparency (%)");

        let dim_unfocus_adj = gtk::Adjustment::new(0.0, 0.0, 100.0, 1.0, 5.0, 0.0);
        let dim_unfocus_row = adw::SpinRow::new(Some(&dim_unfocus_adj), 1.0, 0);
        dim_unfocus_row.set_title("Dim Unfocused Terminal (%)");

        let bold_override_row = adw::SwitchRow::new();
        bold_override_row.set_title("Override Bold Color");

        let bold_color_row = adw::EntryRow::new();
        bold_color_row.set_title("Bold Color (Hex)");

        let bold_is_bright_row = adw::SwitchRow::new();
        bold_is_bright_row.set_title("Show Bold Text in Bright Colors");

        let cursor_override_row = adw::SwitchRow::new();
        cursor_override_row.set_title("Override Cursor Colors");

        let cursor_bg_row = adw::EntryRow::new();
        cursor_bg_row.set_title("Cursor Background (Hex)");

        let cursor_fg_row = adw::EntryRow::new();
        cursor_fg_row.set_title("Cursor Foreground (Hex)");

        let highlight_override_row = adw::SwitchRow::new();
        highlight_override_row.set_title("Override Highlight Colors");

        let highlight_bg_row = adw::EntryRow::new();
        highlight_bg_row.set_title("Highlight Background (Hex)");

        let highlight_fg_row = adw::EntryRow::new();
        highlight_fg_row.set_title("Highlight Foreground (Hex)");

        colors_group.add(&color_preset_row);
        colors_group.add(&use_theme_colors_row);
        colors_group.add(&bg_trans_row);
        colors_group.add(&dim_unfocus_row);
        colors_group.add(&bold_override_row);
        colors_group.add(&bold_color_row);
        colors_group.add(&bold_is_bright_row);
        colors_group.add(&cursor_override_row);
        colors_group.add(&cursor_bg_row);
        colors_group.add(&cursor_fg_row);
        colors_group.add(&highlight_override_row);
        colors_group.add(&highlight_bg_row);
        colors_group.add(&highlight_fg_row);
        profiles_page.add(&colors_group);

        // -------------------------------------------------------------------------
        // 4. Scrolling Tab
        // -------------------------------------------------------------------------
        let scrolling_group = adw::PreferencesGroup::new();
        scrolling_group.set_title("Scrolling Behavior");

        let show_scrollbar_row = adw::SwitchRow::new();
        show_scrollbar_row.set_title("Show Scrollbar");

        let scroll_output_row = adw::SwitchRow::new();
        scroll_output_row.set_title("Scroll on Output");

        let scroll_keystroke_row = adw::SwitchRow::new();
        scroll_keystroke_row.set_title("Scroll on Keystroke");

        let scroll_unlimited_row = adw::SwitchRow::new();
        scroll_unlimited_row.set_title("Unlimited Scrollback");

        let scroll_lines_adj = gtk::Adjustment::new(5000.0, 100.0, 100000.0, 500.0, 1000.0, 0.0);
        let scroll_lines_row = adw::SpinRow::new(Some(&scroll_lines_adj), 1.0, 0);
        scroll_lines_row.set_title("Scrollback Lines");

        scrolling_group.add(&show_scrollbar_row);
        scrolling_group.add(&scroll_output_row);
        scrolling_group.add(&scroll_keystroke_row);
        scrolling_group.add(&scroll_unlimited_row);
        scrolling_group.add(&scroll_lines_row);
        profiles_page.add(&scrolling_group);

        // -------------------------------------------------------------------------
        // 5. Compatibility Tab
        // -------------------------------------------------------------------------
        let compat_group = adw::PreferencesGroup::new();
        compat_group.set_title("Compatibility and Keyboard Emulation");

        let erase_names = [
            "Automatic",
            "ASCII Delete",
            "ASCII Backspace",
            "Delete Sequence",
            "TTY",
        ];
        let backspace_model = gtk::StringList::new(&erase_names);
        let backspace_row = adw::ComboRow::new();
        backspace_row.set_title("Backspace Key Binding");
        backspace_row.set_model(Some(&backspace_model));

        let delete_model = gtk::StringList::new(&erase_names);
        let delete_row = adw::ComboRow::new();
        delete_row.set_title("Delete Key Binding");
        delete_row.set_model(Some(&delete_model));

        let enc_names = ["UTF-8", "ISO-8859-1", "Windows-1252", "US-ASCII"];
        let enc_model = gtk::StringList::new(&enc_names);
        let encoding_row = adw::ComboRow::new();
        encoding_row.set_title("Character Encoding");
        encoding_row.set_model(Some(&enc_model));

        let cjk_names = ["Narrow (1 cell)", "Wide (2 cells)"];
        let cjk_model = gtk::StringList::new(&cjk_names);
        let cjk_row = adw::ComboRow::new();
        cjk_row.set_title("Ambiguous-Width CJK Characters");
        cjk_row.set_model(Some(&cjk_model));

        compat_group.add(&backspace_row);
        compat_group.add(&delete_row);
        compat_group.add(&encoding_row);
        compat_group.add(&cjk_row);
        profiles_page.add(&compat_group);

        // -------------------------------------------------------------------------
        // 6. Badge Tab
        // -------------------------------------------------------------------------
        let badge_group = adw::PreferencesGroup::new();
        badge_group.set_title("Badge Overlay");

        let badge_text_row = adw::EntryRow::new();
        badge_text_row.set_title("Badge Text Format");

        let badge_pos_names = ["Northwest", "Northeast", "Southwest", "Southeast"];
        let badge_pos_model = gtk::StringList::new(&badge_pos_names);
        let badge_pos_row = adw::ComboRow::new();
        badge_pos_row.set_title("Badge Position");
        badge_pos_row.set_model(Some(&badge_pos_model));

        let badge_color_override_row = adw::SwitchRow::new();
        badge_color_override_row.set_title("Override Badge Color");

        let badge_color_row = adw::EntryRow::new();
        badge_color_row.set_title("Badge Color (Hex)");

        let badge_sys_font_row = adw::SwitchRow::new();
        badge_sys_font_row.set_title("Use System Font for Badge");

        let badge_font_row = adw::EntryRow::new();
        badge_font_row.set_title("Custom Badge Font");

        badge_group.add(&badge_text_row);
        badge_group.add(&badge_pos_row);
        badge_group.add(&badge_color_override_row);
        badge_group.add(&badge_color_row);
        badge_group.add(&badge_sys_font_row);
        badge_group.add(&badge_font_row);
        profiles_page.add(&badge_group);

        // -------------------------------------------------------------------------
        // 7. Advanced Tab
        // -------------------------------------------------------------------------
        let adv_group = adw::PreferencesGroup::new();
        adv_group.set_title("Advanced Automation and Silence");

        let silence_row = adw::SwitchRow::new();
        silence_row.set_title("Notify on Silence");

        let silence_thresh_adj = gtk::Adjustment::new(10.0, 1.0, 3600.0, 5.0, 10.0, 0.0);
        let silence_thresh_row = adw::SpinRow::new(Some(&silence_thresh_adj), 1.0, 0);
        silence_thresh_row.set_title("Silence Threshold (seconds)");

        adv_group.add(&silence_row);
        adv_group.add(&silence_thresh_row);
        profiles_page.add(&adv_group);

        // Add Profiles Page to window
        window.add(&profiles_page);

        // -------------------------------------------------------------------------
        // Tab switching visibility logic
        // -------------------------------------------------------------------------
        let groups = [
            general_group.clone(),
            command_group.clone(),
            colors_group.clone(),
            scrolling_group.clone(),
            compat_group.clone(),
            badge_group.clone(),
            adv_group.clone(),
        ];

        let update_tab_visibility = {
            let groups = groups.clone();
            Rc::new(move |selected: u32| {
                for (idx, grp) in groups.iter().enumerate() {
                    grp.set_visible(idx as u32 == selected);
                }
            })
        };

        {
            let utv = Rc::clone(&update_tab_visibility);
            section_combo.connect_selected_notify(move |row| {
                utv(row.selected());
            });
        }
        update_tab_visibility(0);

        // -------------------------------------------------------------------------
        // Profile Data Binding & Live Sync
        // -------------------------------------------------------------------------
        let is_populating = Rc::new(Cell::new(false));
        let selected_id = Rc::new(RefCell::new(
            current_config.borrow().default_profile_id.clone(),
        ));

        let populate_fields = {
            let is_populating = Rc::clone(&is_populating);
            let name_r = prof_name_row.clone();
            let title_r = prof_title_row.clone();
            let cols_r = cols_row.clone();
            let rows_r = rows_row.clone();
            let cell_w_r = cell_w_row.clone();
            let cell_h_r = cell_h_row.clone();
            let margin_r = margin_row.clone();
            let blink_r = blink_mode_row.clone();
            let bold_r = allow_bold_row.clone();
            let rewrap_r = rewrap_row.clone();
            let sys_font_r = system_font_row.clone();
            let font_r = font_row.clone();
            let word_r = word_chars_row.clone();
            let cshape_r = cursor_shape_row.clone();
            let cblink_r = cursor_blink_row.clone();
            let bell_r = bell_row.clone();
            let login_r = login_shell_row.clone();
            let custom_sw_r = use_custom_cmd_row.clone();
            let custom_cmd_r = custom_cmd_row.clone();
            let exit_r = exit_action_row.clone();
            let color_r = color_preset_row.clone();
            let theme_col_r = use_theme_colors_row.clone();
            let bg_trans_r = bg_trans_row.clone();
            let dim_r = dim_unfocus_row.clone();
            let bold_set_r = bold_override_row.clone();
            let bold_hex_r = bold_color_row.clone();
            let bold_bright_r = bold_is_bright_row.clone();
            let cur_set_r = cursor_override_row.clone();
            let cur_bg_r = cursor_bg_row.clone();
            let cur_fg_r = cursor_fg_row.clone();
            let hl_set_r = highlight_override_row.clone();
            let hl_bg_r = highlight_bg_row.clone();
            let hl_fg_r = highlight_fg_row.clone();
            let scr_bar_r = show_scrollbar_row.clone();
            let scr_out_r = scroll_output_row.clone();
            let scr_key_r = scroll_keystroke_row.clone();
            let scr_unl_r = scroll_unlimited_row.clone();
            let scr_lines_r = scroll_lines_row.clone();
            let bs_r = backspace_row.clone();
            let del_r = delete_row.clone();
            let enc_r = encoding_row.clone();
            let cjk_r = cjk_row.clone();
            let badge_txt_r = badge_text_row.clone();
            let badge_pos_r = badge_pos_row.clone();
            let badge_col_set_r = badge_color_override_row.clone();
            let badge_col_r = badge_color_row.clone();
            let badge_sys_r = badge_sys_font_row.clone();
            let badge_font_r = badge_font_row.clone();
            let sil_r = silence_row.clone();
            let sil_th_r = silence_thresh_row.clone();

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

                bold_r.set_active(p.allow_bold);
                rewrap_r.set_active(p.rewrap_on_resize);
                sys_font_r.set_active(p.use_system_font);
                font_r.set_text(p.font.as_deref().unwrap_or("Monospace 11"));
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

                let exit_idx = match p.exit_action {
                    ExitActionPreference::Close => 0,
                    ExitActionPreference::Restart => 1,
                    ExitActionPreference::Hold => 2,
                };
                exit_r.set_selected(exit_idx);

                let color_idx = match p.color_scheme.name.as_str() {
                    "Tilix Light" => 1,
                    "Solarized Dark" => 2,
                    "Monokai" => 3,
                    _ => 0,
                };
                color_r.set_selected(color_idx);
                theme_col_r.set_active(p.use_theme_colors);
                bg_trans_r.set_value(p.background_transparency_percent as f64);
                dim_r.set_value(p.dim_transparency_percent as f64);

                bold_set_r.set_active(p.bold_color_set);
                bold_hex_r.set_text(
                    &p.bold_color
                        .as_ref()
                        .map(|c| c.to_hex())
                        .unwrap_or_default(),
                );
                bold_bright_r.set_active(p.bold_is_bright);

                cur_set_r.set_active(p.cursor_colors_set);
                cur_bg_r.set_text(
                    &p.cursor_background_color
                        .as_ref()
                        .map(|c| c.to_hex())
                        .unwrap_or_default(),
                );
                cur_fg_r.set_text(
                    &p.cursor_foreground_color
                        .as_ref()
                        .map(|c| c.to_hex())
                        .unwrap_or_default(),
                );

                hl_set_r.set_active(p.highlight_colors_set);
                hl_bg_r.set_text(
                    &p.highlight_background_color
                        .as_ref()
                        .map(|c| c.to_hex())
                        .unwrap_or_default(),
                );
                hl_fg_r.set_text(
                    &p.highlight_foreground_color
                        .as_ref()
                        .map(|c| c.to_hex())
                        .unwrap_or_default(),
                );

                scr_bar_r.set_active(p.show_scrollbar);
                scr_out_r.set_active(p.scroll_on_output);
                scr_key_r.set_active(p.scroll_on_keystroke);
                scr_unl_r.set_active(p.scrollback_unlimited);
                scr_lines_r.set_value(p.scrollback_lines.unwrap_or(5000) as f64);

                let bs_idx = match p.backspace_binding {
                    EraseBindingPreference::Auto => 0,
                    EraseBindingPreference::AsciiDelete => 1,
                    EraseBindingPreference::AsciiBackspace => 2,
                    EraseBindingPreference::DeleteSequence => 3,
                    EraseBindingPreference::Tty => 4,
                };
                bs_r.set_selected(bs_idx);

                let del_idx = match p.delete_binding {
                    EraseBindingPreference::Auto => 0,
                    EraseBindingPreference::AsciiDelete => 1,
                    EraseBindingPreference::AsciiBackspace => 2,
                    EraseBindingPreference::DeleteSequence => 3,
                    EraseBindingPreference::Tty => 4,
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
                    CjkWidthPreference::Narrow => 0,
                    CjkWidthPreference::Wide => 1,
                };
                cjk_r.set_selected(cjk_idx);

                badge_txt_r.set_text(&p.badge_text);
                let bpos_idx = match p.badge_position {
                    BadgePosition::Northwest => 0,
                    BadgePosition::Northeast => 1,
                    BadgePosition::Southwest => 2,
                    BadgePosition::Southeast => 3,
                };
                badge_pos_r.set_selected(bpos_idx);
                badge_col_set_r.set_active(p.badge_color_set);
                badge_col_r.set_text(
                    &p.badge_color
                        .as_ref()
                        .map(|c| c.to_hex())
                        .unwrap_or_default(),
                );
                badge_sys_r.set_active(p.badge_use_system_font);
                badge_font_r.set_text(p.badge_font.as_deref().unwrap_or(""));

                sil_r.set_active(p.notify_silence_enabled);
                sil_th_r.set_value(p.notify_silence_threshold as f64);

                is_populating.set(false);
            })
        };

        // Refresh profile selector dropdown list
        let refresh_profiles_dropdown = {
            let config_rc = Rc::clone(&current_config);
            let profile_combo = profile_combo.clone();
            let selected_id = Rc::clone(&selected_id);
            let del_btn = del_btn.clone();
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
                profile_combo.set_model(Some(&str_list));

                del_btn.set_sensitive(cfg.profiles.len() > 1);

                let curr_id = selected_id.borrow().clone();
                let selected_idx = cfg
                    .profiles
                    .iter()
                    .position(|p| p.id == curr_id)
                    .unwrap_or(0);
                profile_combo.set_selected(selected_idx as u32);

                if let Some(prof) = cfg.profiles.get(selected_idx) {
                    *selected_id.borrow_mut() = prof.id.clone();
                    populate_fields(prof);
                }
                is_populating.set(false);
            })
        };

        refresh_profiles_dropdown();

        // -------------------------------------------------------------------------
        // Save current fields back to active profile
        // -------------------------------------------------------------------------
        let save_active_profile = {
            let config_rc = Rc::clone(&current_config);
            let selected_id = Rc::clone(&selected_id);
            let is_populating = Rc::clone(&is_populating);
            let on_change = Rc::clone(&on_change);

            let name_r = prof_name_row.clone();
            let title_r = prof_title_row.clone();
            let cols_r = cols_row.clone();
            let rows_r = rows_row.clone();
            let cell_w_r = cell_w_row.clone();
            let cell_h_r = cell_h_row.clone();
            let margin_r = margin_row.clone();
            let blink_r = blink_mode_row.clone();
            let bold_r = allow_bold_row.clone();
            let rewrap_r = rewrap_row.clone();
            let sys_font_r = system_font_row.clone();
            let font_r = font_row.clone();
            let word_r = word_chars_row.clone();
            let cshape_r = cursor_shape_row.clone();
            let cblink_r = cursor_blink_row.clone();
            let bell_r = bell_row.clone();
            let login_r = login_shell_row.clone();
            let custom_sw_r = use_custom_cmd_row.clone();
            let custom_cmd_r = custom_cmd_row.clone();
            let exit_r = exit_action_row.clone();
            let color_r = color_preset_row.clone();
            let theme_col_r = use_theme_colors_row.clone();
            let bg_trans_r = bg_trans_row.clone();
            let dim_r = dim_unfocus_row.clone();
            let bold_set_r = bold_override_row.clone();
            let bold_hex_r = bold_color_row.clone();
            let bold_bright_r = bold_is_bright_row.clone();
            let cur_set_r = cursor_override_row.clone();
            let cur_bg_r = cursor_bg_row.clone();
            let cur_fg_r = cursor_fg_row.clone();
            let hl_set_r = highlight_override_row.clone();
            let hl_bg_r = highlight_bg_row.clone();
            let hl_fg_r = highlight_fg_row.clone();
            let scr_bar_r = show_scrollbar_row.clone();
            let scr_out_r = scroll_output_row.clone();
            let scr_key_r = scroll_keystroke_row.clone();
            let scr_unl_r = scroll_unlimited_row.clone();
            let scr_lines_r = scroll_lines_row.clone();
            let bs_r = backspace_row.clone();
            let del_r = delete_row.clone();
            let enc_r = encoding_row.clone();
            let cjk_r = cjk_row.clone();
            let badge_txt_r = badge_text_row.clone();
            let badge_pos_r = badge_pos_row.clone();
            let badge_col_set_r = badge_color_override_row.clone();
            let badge_col_r = badge_color_row.clone();
            let badge_sys_r = badge_sys_font_row.clone();
            let badge_font_r = badge_font_row.clone();
            let sil_r = silence_row.clone();
            let sil_th_r = silence_thresh_row.clone();

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

                prof.allow_bold = bold_r.is_active();
                prof.rewrap_on_resize = rewrap_r.is_active();
                prof.use_system_font = sys_font_r.is_active();
                let f_text = font_r.text().to_string();
                prof.font = if f_text.trim().is_empty() {
                    Some("Monospace 11".into())
                } else {
                    Some(f_text)
                };
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

                prof.color_scheme = match color_r.selected() {
                    1 => ColorScheme::tilix_light(),
                    2 => ColorScheme::solarized_dark(),
                    3 => ColorScheme::monokai(),
                    _ => ColorScheme::tilix_dark(),
                };
                prof.use_theme_colors = theme_col_r.is_active();
                prof.background_transparency_percent = bg_trans_r.value() as u32;
                prof.dim_transparency_percent = dim_r.value() as u32;

                prof.bold_color_set = bold_set_r.is_active();
                prof.bold_color = RgbColor::from_hex(&bold_hex_r.text()).ok();
                prof.bold_is_bright = bold_bright_r.is_active();

                prof.cursor_colors_set = cur_set_r.is_active();
                prof.cursor_background_color = RgbColor::from_hex(&cur_bg_r.text()).ok();
                prof.cursor_foreground_color = RgbColor::from_hex(&cur_fg_r.text()).ok();

                prof.highlight_colors_set = hl_set_r.is_active();
                prof.highlight_background_color = RgbColor::from_hex(&hl_bg_r.text()).ok();
                prof.highlight_foreground_color = RgbColor::from_hex(&hl_fg_r.text()).ok();

                prof.show_scrollbar = scr_bar_r.is_active();
                prof.scroll_on_output = scr_out_r.is_active();
                prof.scroll_on_keystroke = scr_key_r.is_active();
                prof.scrollback_unlimited = scr_unl_r.is_active();
                prof.scrollback_lines = Some(scr_lines_r.value() as i64);

                prof.backspace_binding = match bs_r.selected() {
                    1 => EraseBindingPreference::AsciiDelete,
                    2 => EraseBindingPreference::AsciiBackspace,
                    3 => EraseBindingPreference::DeleteSequence,
                    4 => EraseBindingPreference::Tty,
                    _ => EraseBindingPreference::Auto,
                };

                prof.delete_binding = match del_r.selected() {
                    1 => EraseBindingPreference::AsciiDelete,
                    2 => EraseBindingPreference::AsciiBackspace,
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
                prof.badge_color_set = badge_col_set_r.is_active();
                prof.badge_color = RgbColor::from_hex(&badge_col_r.text()).ok();
                prof.badge_use_system_font = badge_sys_r.is_active();
                let b_font = badge_font_r.text().to_string();
                prof.badge_font = if b_font.trim().is_empty() {
                    None
                } else {
                    Some(b_font)
                };

                prof.notify_silence_enabled = sil_r.is_active();
                prof.notify_silence_threshold = sil_th_r.value() as u32;

                let updated = prof.clone();
                if updated.id == cfg.default_profile_id {
                    cfg.default_profile = updated.clone();
                }

                let _ = cfg.save();
                crate::ui::window::apply_profile_to_all_sessions(&updated);
                on_change(&updated);
            })
        };

        // Wire profile selector combo selection
        {
            let config_rc = Rc::clone(&current_config);
            let selected_id = Rc::clone(&selected_id);
            let populate_fields = Rc::clone(&populate_fields);
            let is_populating = Rc::clone(&is_populating);
            let on_change = Rc::clone(&on_change);

            profile_combo.connect_selected_notify(move |combo| {
                if is_populating.get() {
                    return;
                }
                let idx = combo.selected() as usize;
                let cfg = config_rc.borrow();
                if let Some(prof) = cfg.profiles.get(idx) {
                    *selected_id.borrow_mut() = prof.id.clone();
                    populate_fields(prof);
                    crate::ui::window::apply_profile_to_all_sessions(prof);
                    on_change(prof);
                }
            });
        }

        // Connect changes on all row widgets
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
        }

        connect_sync!(prof_name_row, changed_entry);
        connect_sync!(prof_title_row, changed_entry);
        connect_sync!(cols_row, changed_entry);
        connect_sync!(rows_row, changed_entry);
        connect_sync!(cell_w_row, changed_entry);
        connect_sync!(cell_h_row, changed_entry);
        connect_sync!(margin_row, changed_entry);
        connect_sync!(blink_mode_row, notify_selected);
        connect_sync!(allow_bold_row, notify_active);
        connect_sync!(rewrap_row, notify_active);
        connect_sync!(system_font_row, notify_active);
        connect_sync!(font_row, changed_entry);
        connect_sync!(word_chars_row, changed_entry);
        connect_sync!(cursor_shape_row, notify_selected);
        connect_sync!(cursor_blink_row, notify_selected);
        connect_sync!(bell_row, notify_selected);

        connect_sync!(login_shell_row, notify_active);
        connect_sync!(use_custom_cmd_row, notify_active);
        connect_sync!(custom_cmd_row, changed_entry);
        connect_sync!(exit_action_row, notify_selected);

        connect_sync!(color_preset_row, notify_selected);
        connect_sync!(use_theme_colors_row, notify_active);
        connect_sync!(bg_trans_row, changed_entry);
        connect_sync!(dim_unfocus_row, changed_entry);
        connect_sync!(bold_override_row, notify_active);
        connect_sync!(bold_color_row, changed_entry);
        connect_sync!(bold_is_bright_row, notify_active);
        connect_sync!(cursor_override_row, notify_active);
        connect_sync!(cursor_bg_row, changed_entry);
        connect_sync!(cursor_fg_row, changed_entry);
        connect_sync!(highlight_override_row, notify_active);
        connect_sync!(highlight_bg_row, changed_entry);
        connect_sync!(highlight_fg_row, changed_entry);

        connect_sync!(show_scrollbar_row, notify_active);
        connect_sync!(scroll_output_row, notify_active);
        connect_sync!(scroll_keystroke_row, notify_active);
        connect_sync!(scroll_unlimited_row, notify_active);
        connect_sync!(scroll_lines_row, changed_entry);

        connect_sync!(backspace_row, notify_selected);
        connect_sync!(delete_row, notify_selected);
        connect_sync!(encoding_row, notify_selected);
        connect_sync!(cjk_row, notify_selected);

        connect_sync!(badge_text_row, changed_entry);
        connect_sync!(badge_pos_row, notify_selected);
        connect_sync!(badge_color_override_row, notify_active);
        connect_sync!(badge_color_row, changed_entry);
        connect_sync!(badge_sys_font_row, notify_active);
        connect_sync!(badge_font_row, changed_entry);

        connect_sync!(silence_row, notify_active);
        connect_sync!(silence_thresh_row, changed_entry);

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
                if let Ok(cloned) = config_rc.borrow_mut().duplicate_profile(&curr_id) {
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
                if config_rc.borrow_mut().delete_profile(&curr_id).is_ok() {
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
                if config_rc.borrow_mut().set_default_profile(&curr_id).is_ok() {
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

        appearance_page.add(&title_group);
        window.add(&appearance_page);

        // Sync Appearance changes
        {
            let config_rc = Rc::clone(&current_config);
            let w_style = window_style_row.clone();
            let w_handle = wide_handle_row.clone();
            let t_bar = show_tab_bar_row.clone();
            let t_style = title_style_row.clone();
            let t_single = title_show_single_row.clone();

            let save_app = Rc::new(move || {
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
                cfg.window_style = window_style;
                cfg.use_wide_handle = use_wide_handle;
                cfg.show_tab_bar = show_tab_bar;
                cfg.pane_title_style = pane_title_style;
                cfg.pane_title_show_when_single = pane_title_show_when_single;

                let _ = cfg.save();
                crate::ui::window::apply_window_style_to_all_windows(cfg.window_style);
                crate::ui::window::apply_wide_handle_to_all_sessions(cfg.use_wide_handle);
                crate::ui::window::apply_show_tab_bar_to_all_windows(cfg.show_tab_bar);
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
