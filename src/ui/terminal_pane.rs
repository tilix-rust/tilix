use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk4 as gtk;
use gtk::prelude::*;
use vte4 as vte;
use vte::prelude::*;

use crate::model::{
    expand_badge_format, expand_title_format, ColorScheme, CursorBlinkPreference,
    CursorShapePreference, DockPosition, ExitActionPreference, PaneId, Profile,
    SplitOrientation, TitleTokenContext,
};
use crate::pty::{default_env, detect_shell, parse_osc7_uri};

type CloseCallback = Box<dyn Fn(PaneId)>;
type SplitCallback = Box<dyn Fn(PaneId, SplitOrientation)>;
type FocusCallback = Box<dyn Fn(PaneId)>;
type CommitCallback = Box<dyn Fn(PaneId, &str)>;
type TitleCallback = Box<dyn Fn(PaneId, &str)>;
type SyncToggleCallback = Box<dyn Fn(PaneId, bool)>;
type BellCallback = Box<dyn Fn(PaneId)>;
type ChildExitCallback = Box<dyn Fn(PaneId, i32)>;
type DockCallback = Box<dyn Fn(PaneId, PaneId, DockPosition)>;

#[derive(Clone)]
struct PaneWidgets {
    terminal: vte::Terminal,
    scrollbar: gtk::Scrollbar,
    badge_label: gtk::Label,
    margin_line: gtk::Box,
    title_label: gtk::Label,
}

#[derive(Clone)]
pub struct TerminalPane {
    overlay: gtk::Overlay,
    drop_indicator: gtk::Box,
    badge_label: gtk::Label,
    margin_line: gtk::Box,
    scrollbar: gtk::Scrollbar,
    container: gtk::Box,
    header: gtk::Box,
    title_label: gtk::Label,
    sync_btn: gtk::ToggleButton,
    split_h_btn: gtk::Button,
    split_v_btn: gtk::Button,
    close_btn: gtk::Button,
    terminal: vte::Terminal,
    pane_id: PaneId,
    child_pid: Rc<Cell<Option<i32>>>,
    current_directory: Rc<RefCell<Option<PathBuf>>>,
    current_profile: Rc<RefCell<Profile>>,
    is_sync_enabled: Rc<Cell<bool>>,
    is_closing: Rc<Cell<bool>>,
    close_callbacks: Rc<RefCell<Vec<CloseCallback>>>,
    split_callbacks: Rc<RefCell<Vec<SplitCallback>>>,
    focus_callbacks: Rc<RefCell<Vec<FocusCallback>>>,
    commit_callbacks: Rc<RefCell<Vec<CommitCallback>>>,
    title_callbacks: Rc<RefCell<Vec<TitleCallback>>>,
    sync_toggled_callbacks: Rc<RefCell<Vec<SyncToggleCallback>>>,
    bell_callbacks: Rc<RefCell<Vec<BellCallback>>>,
    child_exit_callbacks: Rc<RefCell<Vec<ChildExitCallback>>>,
    dock_callback: Rc<RefCell<Option<DockCallback>>>,
    drag_source_initialized: Rc<Cell<bool>>,
    drop_target_initialized: Rc<Cell<bool>>,
}

impl TerminalPane {
    pub fn new(pane_id: PaneId, initial_directory: Option<&Path>) -> Self {
        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        container.add_css_class("terminal-pane");

        // Header container
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        header.add_css_class("terminal-pane-header");

        let title_label = gtk::Label::new(Some("Terminal"));
        title_label.set_halign(gtk::Align::Start);
        title_label.set_hexpand(true);
        title_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        header.append(&title_label);

        // Header buttons
        let sync_btn = gtk::ToggleButton::new();
        sync_btn.set_icon_name("input-keyboard-symbolic");
        sync_btn.set_tooltip_text(Some("Synchronize Input"));
        sync_btn.add_css_class("flat");
        sync_btn.set_active(true);
        sync_btn.set_focusable(false);

        let split_h_btn = gtk::Button::from_icon_name("object-flip-horizontal-symbolic");
        split_h_btn.set_tooltip_text(Some("Split Right (Ctrl+Shift+R)"));
        split_h_btn.add_css_class("flat");
        split_h_btn.set_focusable(false);

        let split_v_btn = gtk::Button::from_icon_name("object-flip-vertical-symbolic");
        split_v_btn.set_tooltip_text(Some("Split Down (Ctrl+Shift+D)"));
        split_v_btn.add_css_class("flat");
        split_v_btn.set_focusable(false);

        let close_btn = gtk::Button::from_icon_name("window-close-symbolic");
        close_btn.set_tooltip_text(Some("Close Pane (Ctrl+Shift+W)"));
        close_btn.add_css_class("flat");
        close_btn.set_focusable(false);

        header.append(&sync_btn);
        header.append(&split_h_btn);
        header.append(&split_v_btn);
        header.append(&close_btn);

        // Terminal widget & scrollbar in horizontal box
        let terminal = vte::Terminal::new();
        terminal.set_vexpand(true);
        terminal.set_hexpand(true);
        terminal.set_can_focus(true);
        terminal.set_scroll_on_output(true);
        terminal.set_scroll_on_keystroke(true);

        let scrollbar = gtk::Scrollbar::new(gtk::Orientation::Vertical, terminal.vadjustment().as_ref());
        scrollbar.set_visible(true);

        let term_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        term_box.set_vexpand(true);
        term_box.set_hexpand(true);
        term_box.append(&terminal);
        term_box.append(&scrollbar);

        // Wire click gesture on header to focus terminal
        {
            let term_ref = terminal.clone();
            let click_gesture = gtk::GestureClick::new();
            click_gesture.connect_pressed(move |_gesture, _n, _x, _y| {
                term_ref.grab_focus();
            });
            header.add_controller(click_gesture);
        }

        container.append(&header);
        container.append(&term_box);

        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&container));

        let drop_indicator = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        drop_indicator.add_css_class("drop-indicator-overlay");
        drop_indicator.set_can_target(false);
        drop_indicator.set_visible(false);
        overlay.add_overlay(&drop_indicator);

        let margin_line = gtk::Box::new(gtk::Orientation::Vertical, 0);
        margin_line.add_css_class("terminal-margin-line");
        margin_line.set_halign(gtk::Align::Start);
        margin_line.set_valign(gtk::Align::Fill);
        margin_line.set_can_target(false);
        margin_line.set_visible(false);
        overlay.add_overlay(&margin_line);

        let badge_label = gtk::Label::new(None);
        badge_label.add_css_class("terminal-badge");
        badge_label.set_can_target(false);
        badge_label.set_visible(false);
        overlay.add_overlay(&badge_label);

        let pane_widgets = PaneWidgets {
            terminal: terminal.clone(),
            scrollbar: scrollbar.clone(),
            badge_label: badge_label.clone(),
            margin_line: margin_line.clone(),
            title_label: title_label.clone(),
        };

        let child_pid: Rc<Cell<Option<i32>>> = Rc::new(Cell::new(None));
        let current_directory = Rc::new(RefCell::new(initial_directory.map(|p| p.to_path_buf())));
        let current_profile = Rc::new(RefCell::new(Profile::default()));
        let is_sync_enabled = Rc::new(Cell::new(true));
        let is_closing = Rc::new(Cell::new(false));
        let close_callbacks: Rc<RefCell<Vec<CloseCallback>>> = Rc::new(RefCell::new(Vec::new()));
        let split_callbacks: Rc<RefCell<Vec<SplitCallback>>> = Rc::new(RefCell::new(Vec::new()));
        let focus_callbacks: Rc<RefCell<Vec<FocusCallback>>> = Rc::new(RefCell::new(Vec::new()));
        let commit_callbacks: Rc<RefCell<Vec<CommitCallback>>> = Rc::new(RefCell::new(Vec::new()));
        let title_callbacks: Rc<RefCell<Vec<TitleCallback>>> = Rc::new(RefCell::new(Vec::new()));
        let sync_toggled_callbacks: Rc<RefCell<Vec<SyncToggleCallback>>> = Rc::new(RefCell::new(Vec::new()));
        let bell_callbacks: Rc<RefCell<Vec<BellCallback>>> = Rc::new(RefCell::new(Vec::new()));
        let child_exit_callbacks: Rc<RefCell<Vec<ChildExitCallback>>> = Rc::new(RefCell::new(Vec::new()));

        // Wire sync button
        {
            let is_sync = Rc::clone(&is_sync_enabled);
            let callbacks = Rc::clone(&sync_toggled_callbacks);
            sync_btn.connect_toggled(move |btn| {
                let active = btn.is_active();
                is_sync.set(active);
                if let Ok(list) = callbacks.try_borrow() {
                    for cb in list.iter() {
                        cb(pane_id, active);
                    }
                }
            });
        }

        // Wire close button
        {
            let is_closing = Rc::clone(&is_closing);
            let callbacks = Rc::clone(&close_callbacks);
            close_btn.connect_clicked(move |_| {
                if is_closing.get() {
                    return;
                }
                is_closing.set(true);
                if let Ok(list) = callbacks.try_borrow() {
                    for cb in list.iter() {
                        cb(pane_id);
                    }
                }
            });
        }

        // Wire split horizontal button
        {
            let callbacks = Rc::clone(&split_callbacks);
            split_h_btn.connect_clicked(move |_| {
                if let Ok(list) = callbacks.try_borrow() {
                    for cb in list.iter() {
                        cb(pane_id, SplitOrientation::Horizontal);
                    }
                }
            });
        }

        // Wire split vertical button
        {
            let callbacks = Rc::clone(&split_callbacks);
            split_v_btn.connect_clicked(move |_| {
                if let Ok(list) = callbacks.try_borrow() {
                    for cb in list.iter() {
                        cb(pane_id, SplitOrientation::Vertical);
                    }
                }
            });
        }

        // Wire child exit with exit action handling
        {
            let close_cbs = Rc::clone(&close_callbacks);
            let exit_cbs = Rc::clone(&child_exit_callbacks);
            let is_closing = Rc::clone(&is_closing);
            let profile_rc = Rc::clone(&current_profile);
            let title_lbl = title_label.clone();
            let current_dir = Rc::clone(&current_directory);
            let pid_cell = Rc::clone(&child_pid);

            terminal.connect_child_exited(move |term, status| {
                pid_cell.set(None);
                if is_closing.get() {
                    return;
                }
                if let Ok(exit_list) = exit_cbs.try_borrow() {
                    for cb in exit_list.iter() {
                        cb(pane_id, status);
                    }
                }

                let action = profile_rc.borrow().exit_action;
                match action {
                    ExitActionPreference::Close => {
                        if let Ok(list) = close_cbs.try_borrow() {
                            for cb in list.iter() {
                                cb(pane_id);
                            }
                        }
                    }
                    ExitActionPreference::Restart => {
                        let dir = current_dir.borrow().clone();
                        Self::spawn_shell_process(term, &profile_rc.borrow(), dir.as_deref(), &pid_cell);
                    }
                    ExitActionPreference::Hold => {
                        let curr_title = title_lbl.text();
                        title_lbl.set_text(&format!("{} [Process exited: {}]", curr_title, status));
                    }
                }
            });
        }

        // Wire bell
        {
            let callbacks = Rc::clone(&bell_callbacks);
            let is_closing = Rc::clone(&is_closing);
            terminal.connect_bell(move |_term| {
                if is_closing.get() {
                    return;
                }
                if let Ok(list) = callbacks.try_borrow() {
                    for cb in list.iter() {
                        cb(pane_id);
                    }
                }
            });
        }

        // Wire OSC 7 current directory uri changes and automatic switch evaluation
        {
            let cwd_clone = Rc::clone(&current_directory);
            let prof_rc = Rc::clone(&current_profile);
            let widgets = pane_widgets.clone();
            let callbacks = Rc::clone(&title_callbacks);
            terminal.connect_current_directory_uri_changed(move |term| {
                if let Some(uri) = term.current_directory_uri() {
                    if let Some(path) = parse_osc7_uri(&uri) {
                        if let Ok(mut cwd) = cwd_clone.try_borrow_mut() {
                            *cwd = Some(path.clone());
                        }
                        Self::check_auto_switch_static(
                            &prof_rc,
                            Some(&path),
                            pane_id,
                            &widgets,
                        );
                        if let Ok(list) = callbacks.try_borrow() {
                            let title = term.window_title().map(|s| s.to_string()).unwrap_or_else(|| "Terminal".to_string());
                            for cb in list.iter() {
                                cb(pane_id, &title);
                            }
                        }
                    }
                }
            });
        }

        // Wire window title change and automatic switch evaluation
        {
            let label_clone = title_label.clone();
            let callbacks = Rc::clone(&title_callbacks);
            let prof_rc = Rc::clone(&current_profile);
            let cwd_clone = Rc::clone(&current_directory);
            let widgets = pane_widgets.clone();
            terminal.connect_window_title_changed(move |term| {
                if let Some(title) = term.window_title() {
                    label_clone.set_text(&title);
                    if let Ok(list) = callbacks.try_borrow() {
                        for cb in list.iter() {
                            cb(pane_id, &title);
                        }
                    }
                    let dir = cwd_clone.borrow().clone();
                    Self::check_auto_switch_static(
                        &prof_rc,
                        dir.as_deref(),
                        pane_id,
                        &widgets,
                    );
                }
            });
        }

        // Wire focus notification
        {
            let callbacks = Rc::clone(&focus_callbacks);
            let focus_ctrl = gtk::EventControllerFocus::new();
            focus_ctrl.connect_enter(move |_| {
                if let Ok(list) = callbacks.try_borrow() {
                    for cb in list.iter() {
                        cb(pane_id);
                    }
                }
            });
            terminal.add_controller(focus_ctrl);
        }

        // Wire commit notification (for synchronized input)
        {
            let callbacks = Rc::clone(&commit_callbacks);
            terminal.connect_commit(move |_term, text, _size| {
                if let Ok(list) = callbacks.try_borrow() {
                    for cb in list.iter() {
                        cb(pane_id, text);
                    }
                }
            });
        }

        // Spawn initial shell process asynchronously
        Self::spawn_shell_process(&terminal, &current_profile.borrow(), initial_directory, &child_pid);

        Self {
            overlay,
            drop_indicator,
            badge_label,
            margin_line,
            scrollbar,
            container,
            header,
            title_label,
            sync_btn,
            split_h_btn,
            split_v_btn,
            close_btn,
            terminal,
            pane_id,
            child_pid,
            current_directory,
            current_profile,
            is_sync_enabled,
            is_closing,
            close_callbacks,
            split_callbacks,
            focus_callbacks,
            commit_callbacks,
            title_callbacks,
            sync_toggled_callbacks,
            bell_callbacks,
            child_exit_callbacks,
            dock_callback: Rc::new(RefCell::new(None)),
            drag_source_initialized: Rc::new(Cell::new(false)),
            drop_target_initialized: Rc::new(Cell::new(false)),
        }
    }

    fn spawn_shell_process(
        terminal: &vte::Terminal,
        profile: &Profile,
        directory: Option<&Path>,
        child_pid: &Rc<Cell<Option<i32>>>,
    ) {
        let shell = detect_shell();
        let env_vars = default_env();
        let env_refs: Vec<&str> = env_vars.iter().map(|s| s.as_str()).collect();
        let init_dir_str = directory.and_then(|p| p.to_str());
        let (_cmd, argv) = crate::pty::shell::build_spawn_args(profile, &shell);
        let argv_refs: Vec<&str> = argv.iter().map(|s| s.as_str()).collect();

        let pid_cell = Rc::clone(child_pid);
        terminal.spawn_async(
            vte::PtyFlags::DEFAULT,
            init_dir_str,
            &argv_refs,
            &env_refs,
            glib::SpawnFlags::DEFAULT,
            || {},
            -1,
            gio::Cancellable::NONE,
            move |res| {
                match res {
                    Ok(pid) => {
                        pid_cell.set(Some(pid.0));
                    }
                    Err(e) => {
                        glib::g_warning!("Tilix", "Failed to spawn shell: {}", e);
                    }
                }
            },
        );
    }

    fn check_auto_switch_static(
        profile_rc: &Rc<RefCell<Profile>>,
        directory: Option<&Path>,
        pane_id: PaneId,
        widgets: &PaneWidgets,
    ) {
        let curr_prof = profile_rc.borrow().clone();
        if curr_prof.automatic_switch.is_empty() {
            return;
        }
        let hostname = glib::host_name();
        let empty_path = Path::new("");
        let dir = directory.unwrap_or(empty_path);
        for rule in &curr_prof.automatic_switch {
            if rule.matches(&hostname, dir) && rule.profile_id != curr_prof.id {
                let cfg = crate::model::AppConfig::load();
                if let Some(target) = cfg.get_profile(&rule.profile_id) {
                    *profile_rc.borrow_mut() = target.clone();
                    Self::apply_profile_to_widgets(
                        target,
                        pane_id,
                        directory,
                        widgets,
                    );
                    break;
                }
            }
        }
    }

    pub fn close_button(&self) -> &gtk::Button {
        &self.close_btn
    }

    pub fn split_h_button(&self) -> &gtk::Button {
        &self.split_h_btn
    }

    pub fn split_v_button(&self) -> &gtk::Button {
        &self.split_v_btn
    }

    pub fn sync_button(&self) -> &gtk::ToggleButton {
        &self.sync_btn
    }

    pub fn scrollbar(&self) -> &gtk::Scrollbar {
        &self.scrollbar
    }

    pub fn is_scrollbar_visible(&self) -> bool {
        self.scrollbar.get_visible()
    }

    pub fn badge_label(&self) -> &gtk::Label {
        &self.badge_label
    }

    pub fn margin_line(&self) -> &gtk::Box {
        &self.margin_line
    }

    pub fn current_profile(&self) -> Profile {
        self.current_profile.borrow().clone()
    }

    pub fn pane_id(&self) -> PaneId {
        self.pane_id
    }

    pub fn title(&self) -> String {
        self.title_label.text().to_string()
    }

    pub fn title_label(&self) -> &gtk::Label {
        &self.title_label
    }

    pub fn is_closing(&self) -> bool {
        self.is_closing.get()
    }

    pub fn close(&self) {
        self.is_closing.set(true);
    }

    pub fn widget(&self) -> &gtk::Widget {
        self.overlay.upcast_ref()
    }

    pub fn overlay(&self) -> &gtk::Overlay {
        &self.overlay
    }

    pub fn terminal(&self) -> &vte::Terminal {
        &self.terminal
    }

    pub fn header(&self) -> &gtk::Box {
        &self.header
    }

    pub fn is_sync_enabled(&self) -> bool {
        self.is_sync_enabled.get()
    }

    pub fn set_sync_enabled(&self, enabled: bool) {
        self.is_sync_enabled.set(enabled);
        self.sync_btn.set_active(enabled);
    }

    pub fn feed_child(&self, data: &[u8]) {
        self.terminal.feed_child(data);
    }

    pub fn set_active(&self, active: bool) {
        if active {
            self.container.add_css_class("active-pane");
            self.terminal.set_opacity(1.0);
        } else {
            self.container.remove_css_class("active-pane");
            let dim = self.current_profile.borrow().dim_transparency_percent;
            if dim > 0 {
                self.terminal.set_opacity(1.0 - (dim.min(100) as f64 / 100.0));
            } else {
                self.terminal.set_opacity(1.0);
            }
        }
    }

    pub fn set_title(&self, title: &str) {
        self.title_label.set_text(title);
    }

    pub fn grab_focus(&self) {
        self.terminal.grab_focus();
    }

    pub fn connect_close<F: Fn(PaneId) + 'static>(&self, f: F) {
        self.close_callbacks.borrow_mut().push(Box::new(f));
    }

    pub fn connect_split<F: Fn(PaneId, SplitOrientation) + 'static>(&self, f: F) {
        self.split_callbacks.borrow_mut().push(Box::new(f));
    }

    pub fn connect_focus<F: Fn(PaneId) + 'static>(&self, f: F) {
        self.focus_callbacks.borrow_mut().push(Box::new(f));
    }

    pub fn connect_commit<F: Fn(PaneId, &str) + 'static>(&self, f: F) {
        self.commit_callbacks.borrow_mut().push(Box::new(f));
    }

    pub fn connect_title_changed<F: Fn(PaneId, &str) + 'static>(&self, f: F) {
        self.title_callbacks.borrow_mut().push(Box::new(f));
    }

    pub fn connect_sync_toggled<F: Fn(PaneId, bool) + 'static>(&self, f: F) {
        self.sync_toggled_callbacks.borrow_mut().push(Box::new(f));
    }

    pub fn apply_color_scheme(&self, scheme: &ColorScheme) {
        let to_gdk = |c: &crate::model::RgbColor| {
            gtk::gdk::RGBA::builder()
                .red(c.red as f32)
                .green(c.green as f32)
                .blue(c.blue as f32)
                .alpha(c.alpha as f32)
                .build()
        };

        let fg = to_gdk(&scheme.foreground);
        let bg = to_gdk(&scheme.background);
        let palette_gdk: Vec<gtk::gdk::RGBA> = scheme.palette.iter().map(to_gdk).collect();
        let palette_refs: Vec<&gtk::gdk::RGBA> = palette_gdk.iter().collect();

        self.terminal
            .set_colors(Some(&fg), Some(&bg), &palette_refs);

        if let Some(ref c) = scheme.cursor {
            let cursor_rgba = to_gdk(c);
            self.terminal.set_color_cursor(Some(&cursor_rgba));
        } else {
            self.terminal.set_color_cursor(None);
        }

        if let Some(ref c) = scheme.cursor_foreground {
            let cursor_fg_rgba = to_gdk(c);
            self.terminal
                .set_color_cursor_foreground(Some(&cursor_fg_rgba));
        } else {
            self.terminal.set_color_cursor_foreground(None);
        }
    }

    fn apply_profile_to_widgets(
        profile: &Profile,
        pane_id: PaneId,
        directory: Option<&Path>,
        widgets: &PaneWidgets,
    ) {
        widgets.scrollbar.set_visible(profile.show_scrollbar);

        // Font
        if profile.use_system_font {
            let font_desc = gtk::pango::FontDescription::from_string("Monospace 11");
            widgets.terminal.set_font(Some(&font_desc));
        } else if let Some(ref font_name) = profile.font {
            let font_desc = gtk::pango::FontDescription::from_string(font_name);
            widgets.terminal.set_font(Some(&font_desc));
        }

        // Cell scale
        let w_scale = profile.cell_width_scale.clamp(1.0, 2.0);
        let h_scale = profile.cell_height_scale.clamp(1.0, 2.0);
        widgets.terminal.set_cell_width_scale(w_scale);
        widgets.terminal.set_cell_height_scale(h_scale);

        // Text blink
        widgets.terminal.set_text_blink_mode(profile.text_blink_mode.into());
        widgets.terminal.set_bold_is_bright(profile.bold_is_bright);

        let to_gdk = |c: &crate::model::RgbColor| {
            gtk::gdk::RGBA::builder()
                .red(c.red as f32)
                .green(c.green as f32)
                .blue(c.blue as f32)
                .alpha(c.alpha as f32)
                .build()
        };

        // Bold color
        if profile.bold_color_set {
            if let Some(ref c) = profile.bold_color {
                let rgba = to_gdk(c);
                widgets.terminal.set_color_bold(Some(&rgba));
            } else {
                widgets.terminal.set_color_bold(None);
            }
        } else {
            widgets.terminal.set_color_bold(None);
        }

        // Cursor color
        if profile.cursor_colors_set {
            if let Some(ref c) = profile.cursor_background_color {
                let rgba = to_gdk(c);
                widgets.terminal.set_color_cursor(Some(&rgba));
            } else {
                widgets.terminal.set_color_cursor(None);
            }
            if let Some(ref c) = profile.cursor_foreground_color {
                let rgba = to_gdk(c);
                widgets.terminal.set_color_cursor_foreground(Some(&rgba));
            } else {
                widgets.terminal.set_color_cursor_foreground(None);
            }
        } else if let Some(ref c) = profile.color_scheme.cursor {
            let cursor_rgba = to_gdk(c);
            widgets.terminal.set_color_cursor(Some(&cursor_rgba));
            if let Some(ref c_fg) = profile.color_scheme.cursor_foreground {
                let fg_rgba = to_gdk(c_fg);
                widgets.terminal.set_color_cursor_foreground(Some(&fg_rgba));
            } else {
                widgets.terminal.set_color_cursor_foreground(None);
            }
        } else {
            widgets.terminal.set_color_cursor(None);
            widgets.terminal.set_color_cursor_foreground(None);
        }

        // Highlight color
        if profile.highlight_colors_set {
            if let Some(ref c) = profile.highlight_background_color {
                let rgba = to_gdk(c);
                widgets.terminal.set_color_highlight(Some(&rgba));
            } else {
                widgets.terminal.set_color_highlight(None);
            }
            if let Some(ref c) = profile.highlight_foreground_color {
                let rgba = to_gdk(c);
                widgets.terminal.set_color_highlight_foreground(Some(&rgba));
            } else {
                widgets.terminal.set_color_highlight_foreground(None);
            }
        } else {
            widgets.terminal.set_color_highlight(None);
            widgets.terminal.set_color_highlight_foreground(None);
        }

        // Background transparency & colors
        let fg = to_gdk(&profile.color_scheme.foreground);
        let mut bg = to_gdk(&profile.color_scheme.background);
        if profile.background_transparency_percent > 0 {
            let alpha = (100 - profile.background_transparency_percent.min(100)) as f32 / 100.0;
            bg = gtk::gdk::RGBA::builder()
                .red(bg.red())
                .green(bg.green())
                .blue(bg.blue())
                .alpha(alpha)
                .build();
        }
        let palette_gdk: Vec<gtk::gdk::RGBA> = profile.color_scheme.palette.iter().map(to_gdk).collect();
        let palette_refs: Vec<&gtk::gdk::RGBA> = palette_gdk.iter().collect();
        widgets.terminal.set_colors(Some(&fg), Some(&bg), &palette_refs);

        // Cursor shape & blink
        match profile.cursor_shape {
            CursorShapePreference::Block => widgets.terminal.set_cursor_shape(vte::CursorShape::Block),
            CursorShapePreference::IBeam => widgets.terminal.set_cursor_shape(vte::CursorShape::Ibeam),
            CursorShapePreference::Underline => widgets.terminal.set_cursor_shape(vte::CursorShape::Underline),
        }
        match profile.cursor_blink {
            CursorBlinkPreference::System => widgets.terminal.set_cursor_blink_mode(vte::CursorBlinkMode::System),
            CursorBlinkPreference::On => widgets.terminal.set_cursor_blink_mode(vte::CursorBlinkMode::On),
            CursorBlinkPreference::Off => widgets.terminal.set_cursor_blink_mode(vte::CursorBlinkMode::Off),
        }

        // Scrollback
        if profile.scrollback_unlimited {
            widgets.terminal.set_scrollback_lines(-1);
        } else if let Some(lines) = profile.scrollback_lines {
            widgets.terminal.set_scrollback_lines(lines);
        }
        widgets.terminal.set_scroll_on_output(profile.scroll_on_output);
        widgets.terminal.set_scroll_on_keystroke(profile.scroll_on_keystroke);

        // Compatibility
        widgets.terminal.set_backspace_binding(profile.backspace_binding.into());
        widgets.terminal.set_delete_binding(profile.delete_binding.into());
        widgets.terminal.set_cjk_ambiguous_width(profile.cjk_utf8_ambiguous_width.to_width());
        widgets.terminal.set_word_char_exceptions(&profile.select_by_word_chars);

        // Margin guide line
        widgets.margin_line.set_visible(profile.draw_margin > 0);

        // Token expansion context
        let current_title = widgets.title_label.text().to_string();
        let ctx = TitleTokenContext {
            id: pane_id.0,
            title: &current_title,
            profile_name: &profile.name,
            directory,
            app_name: "Tilix",
        };

        // Title format
        let expanded_title = expand_title_format(&profile.terminal_title, &ctx);
        widgets.title_label.set_text(&expanded_title);

        // Badge overlay
        if profile.badge_text.trim().is_empty() {
            widgets.badge_label.set_visible(false);
        } else {
            let expanded_badge = expand_badge_format(&profile.badge_text, &ctx);
            widgets.badge_label.set_text(&expanded_badge);
            match profile.badge_position {
                crate::model::BadgePosition::Northwest => {
                    widgets.badge_label.set_halign(gtk::Align::Start);
                    widgets.badge_label.set_valign(gtk::Align::Start);
                }
                crate::model::BadgePosition::Northeast => {
                    widgets.badge_label.set_halign(gtk::Align::End);
                    widgets.badge_label.set_valign(gtk::Align::Start);
                }
                crate::model::BadgePosition::Southwest => {
                    widgets.badge_label.set_halign(gtk::Align::Start);
                    widgets.badge_label.set_valign(gtk::Align::End);
                }
                crate::model::BadgePosition::Southeast => {
                    widgets.badge_label.set_halign(gtk::Align::End);
                    widgets.badge_label.set_valign(gtk::Align::End);
                }
            }
            widgets.badge_label.set_visible(true);
        }
    }

    pub fn apply_profile(&self, profile: &Profile) {
        *self.current_profile.borrow_mut() = profile.clone();
        let dir = self.current_directory();
        let widgets = PaneWidgets {
            terminal: self.terminal.clone(),
            scrollbar: self.scrollbar.clone(),
            badge_label: self.badge_label.clone(),
            margin_line: self.margin_line.clone(),
            title_label: self.title_label.clone(),
        };
        Self::apply_profile_to_widgets(
            profile,
            self.pane_id,
            dir.as_deref(),
            &widgets,
        );
    }

    pub fn check_automatic_profile_switching(&self) {
        let curr_prof = self.current_profile.borrow().clone();
        if curr_prof.automatic_switch.is_empty() {
            return;
        }
        let hostname = glib::host_name();
        let dir = self.current_directory();
        let empty_path = Path::new("");
        let dir_ref = dir.as_deref().unwrap_or(empty_path);
        for rule in &curr_prof.automatic_switch {
            if rule.matches(&hostname, dir_ref) && rule.profile_id != curr_prof.id {
                let cfg = crate::model::AppConfig::load();
                if let Some(target) = cfg.get_profile(&rule.profile_id) {
                    self.apply_profile(target);
                    break;
                }
            }
        }
    }

    pub fn current_directory(&self) -> Option<PathBuf> {
        if let Some(dir) = self.current_directory.borrow().clone() {
            return Some(dir);
        }
        #[cfg(target_os = "linux")]
        if let Some(pid) = self.child_pid.get() {
            if let Ok(target) = std::fs::read_link(format!("/proc/{}/cwd", pid)) {
                return Some(target);
            }
        }
        None
    }

    pub fn set_current_directory(&self, dir: Option<PathBuf>) {
        *self.current_directory.borrow_mut() = dir;
    }

    pub fn child_pid(&self) -> Option<i32> {
        self.child_pid.get()
    }

    pub fn connect_bell<F: Fn(PaneId) + 'static>(&self, f: F) {
        self.bell_callbacks.borrow_mut().push(Box::new(f));
    }

    pub fn connect_child_exited<F: Fn(PaneId, i32) + 'static>(&self, f: F) {
        self.child_exit_callbacks.borrow_mut().push(Box::new(f));
    }

    pub fn setup_drag_source(&self) {
        if self.drag_source_initialized.get() {
            return;
        }
        self.drag_source_initialized.set(true);
        crate::ui::dnd::setup_pane_drag_source(&self.header, self.clone());
    }

    pub fn setup_drop_target<F: Fn(PaneId, PaneId, DockPosition) + 'static>(&self, on_dock: F) {
        *self.dock_callback.borrow_mut() = Some(Box::new(on_dock));
        if self.drop_target_initialized.get() {
            return;
        }
        self.drop_target_initialized.set(true);
        let dock_cb = Rc::clone(&self.dock_callback);
        let pane = self.clone();
        crate::ui::dnd::setup_pane_drop_target(&self.overlay, pane, move |src, dest, pos| {
            if let Some(ref cb) = *dock_cb.borrow() {
                cb(src, dest, pos);
            }
        });
    }

    pub fn set_header_visible(&self, visible: bool) {
        self.header.set_visible(visible);
    }

    pub fn is_header_visible(&self) -> bool {
        self.header.get_visible()
    }

    pub fn show_drop_indicator(&self, position: DockPosition) {
        match position {
            DockPosition::Top => {
                self.drop_indicator.set_halign(gtk::Align::Fill);
                self.drop_indicator.set_valign(gtk::Align::Start);
                let h = (self.overlay.height() / 2).max(20);
                self.drop_indicator.set_size_request(-1, h);
            }
            DockPosition::Bottom => {
                self.drop_indicator.set_halign(gtk::Align::Fill);
                self.drop_indicator.set_valign(gtk::Align::End);
                let h = (self.overlay.height() / 2).max(20);
                self.drop_indicator.set_size_request(-1, h);
            }
            DockPosition::Left => {
                self.drop_indicator.set_halign(gtk::Align::Start);
                self.drop_indicator.set_valign(gtk::Align::Fill);
                let w = (self.overlay.width() / 2).max(20);
                self.drop_indicator.set_size_request(w, -1);
            }
            DockPosition::Right => {
                self.drop_indicator.set_halign(gtk::Align::End);
                self.drop_indicator.set_valign(gtk::Align::Fill);
                let w = (self.overlay.width() / 2).max(20);
                self.drop_indicator.set_size_request(w, -1);
            }
            DockPosition::Center => {
                self.drop_indicator.set_halign(gtk::Align::Fill);
                self.drop_indicator.set_valign(gtk::Align::Fill);
                self.drop_indicator.set_size_request(-1, -1);
            }
        }
        self.drop_indicator.set_visible(true);
    }

    pub fn hide_drop_indicator(&self) {
        self.drop_indicator.set_visible(false);
    }

    pub fn is_drop_indicator_visible(&self) -> bool {
        self.drop_indicator.get_visible()
    }

    pub fn clear_callbacks(&self) {
        self.close_callbacks.borrow_mut().clear();
        self.split_callbacks.borrow_mut().clear();
        self.focus_callbacks.borrow_mut().clear();
        self.commit_callbacks.borrow_mut().clear();
        self.title_callbacks.borrow_mut().clear();
        self.sync_toggled_callbacks.borrow_mut().clear();
        self.bell_callbacks.borrow_mut().clear();
        self.child_exit_callbacks.borrow_mut().clear();
        self.dock_callback.borrow_mut().take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_pane_header_visibility() {
        crate::ui::window::run_gtk_test(|| {
            let pane = TerminalPane::new(PaneId(1), None);
            assert!(pane.is_header_visible());

            pane.set_header_visible(false);
            assert!(!pane.is_header_visible());

            pane.set_header_visible(true);
            assert!(pane.is_header_visible());
        });
    }

    #[test]
    fn test_terminal_pane_drop_indicator_visibility() {
        crate::ui::window::run_gtk_test(|| {
            let pane = TerminalPane::new(PaneId(1), None);
            assert!(!pane.is_drop_indicator_visible());

            pane.show_drop_indicator(DockPosition::Top);
            assert!(pane.is_drop_indicator_visible());

            pane.hide_drop_indicator();
            assert!(!pane.is_drop_indicator_visible());

            pane.show_drop_indicator(DockPosition::Center);
            assert!(pane.is_drop_indicator_visible());

            pane.hide_drop_indicator();
            assert!(!pane.is_drop_indicator_visible());
        });
    }

    #[test]
    fn test_terminal_pane_clear_callbacks() {
        crate::ui::window::run_gtk_test(|| {
            let pane = TerminalPane::new(PaneId(1), None);
            let called = Rc::new(Cell::new(false));
            let called_c = Rc::clone(&called);
            pane.connect_close(move |_| {
                called_c.set(true);
            });

            pane.clear_callbacks();
            pane.close_button().emit_clicked();
            assert!(!called.get());
        });
    }

    #[test]
    fn test_terminal_pane_scrollbar_toggle() {
        crate::ui::window::run_gtk_test(|| {
            let pane = TerminalPane::new(PaneId(1), None);
            let mut prof = Profile {
                show_scrollbar: true,
                ..Default::default()
            };
            pane.apply_profile(&prof);
            assert!(pane.is_scrollbar_visible());

            prof.show_scrollbar = false;
            pane.apply_profile(&prof);
            assert!(!pane.is_scrollbar_visible());
        });
    }

    #[test]
    fn test_terminal_pane_badge_overlay() {
        crate::ui::window::run_gtk_test(|| {
            let pane = TerminalPane::new(PaneId(1), None);
            let badge = pane.badge_label();
            assert!(!badge.can_target());
            assert!(!badge.get_visible());

            let prof = Profile {
                badge_text: "DEV ${id}".into(),
                badge_position: crate::model::BadgePosition::Southwest,
                ..Default::default()
            };
            pane.apply_profile(&prof);

            assert!(badge.get_visible());
            assert_eq!(badge.text().as_str(), "DEV 1");
            assert_eq!(badge.halign(), gtk::Align::Start);
            assert_eq!(badge.valign(), gtk::Align::End);
        });
    }

    #[test]
    fn test_terminal_pane_apply_extended_profile() {
        crate::ui::window::run_gtk_test(|| {
            let pane = TerminalPane::new(PaneId(1), None);
            let prof = Profile {
                cell_width_scale: 1.3,
                cell_height_scale: 1.2,
                background_transparency_percent: 25,
                dim_transparency_percent: 30,
                draw_margin: 80,
                terminal_title: "MyTitle [${id}]".into(),
                ..Default::default()
            };

            pane.apply_profile(&prof);

            assert_eq!(pane.title(), "MyTitle [1]");
            assert!(pane.margin_line().get_visible());
            assert_eq!(pane.current_profile().dim_transparency_percent, 30);

            // Test dimming on inactive
            pane.set_active(false);
            assert!((pane.terminal().opacity() - 0.7).abs() < 0.01);
            pane.set_active(true);
            assert_eq!(pane.terminal().opacity(), 1.0);
        });
    }

    #[test]
    fn test_terminal_pane_exit_action_hold() {
        crate::ui::window::run_gtk_test(|| {
            let pane = TerminalPane::new(PaneId(1), None);
            let prof = Profile {
                exit_action: ExitActionPreference::Hold,
                ..Default::default()
            };
            pane.apply_profile(&prof);

            let close_called = Rc::new(Cell::new(false));
            let close_called_c = Rc::clone(&close_called);
            pane.connect_close(move |_| {
                close_called_c.set(true);
            });

            // Simulate child exit with status 0
            pane.terminal().emit_by_name::<()>("child-exited", &[&0i32]);

            // Hold should NOT trigger close callbacks
            assert!(!close_called.get());
            assert!(pane.title().contains("[Process exited: 0]"));
        });
    }
}
