use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk4 as gtk;
use gtk::prelude::*;
use vte4 as vte;
use vte::prelude::*;

use crate::model::{ColorScheme, CursorBlinkPreference, CursorShapePreference, PaneId, Profile, SplitOrientation};
use crate::pty::{default_env, detect_shell, parse_osc7_uri};

type CloseCallback = Box<dyn Fn(PaneId)>;
type SplitCallback = Box<dyn Fn(PaneId, SplitOrientation)>;
type FocusCallback = Box<dyn Fn(PaneId)>;
type CommitCallback = Box<dyn Fn(PaneId, &str)>;
type TitleCallback = Box<dyn Fn(PaneId, &str)>;
type SyncToggleCallback = Box<dyn Fn(PaneId, bool)>;
type BellCallback = Box<dyn Fn(PaneId)>;
type ChildExitCallback = Box<dyn Fn(PaneId, i32)>;

#[derive(Clone)]
pub struct TerminalPane {
    container: gtk::Box,
    header: gtk::Box,
    title_label: gtk::Label,
    sync_btn: gtk::ToggleButton,
    split_h_btn: gtk::Button,
    split_v_btn: gtk::Button,
    close_btn: gtk::Button,
    terminal: vte::Terminal,
    pane_id: PaneId,
    current_directory: Rc<RefCell<Option<PathBuf>>>,
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
}

impl TerminalPane {
    pub fn new(pane_id: PaneId, initial_directory: Option<&Path>) -> Self {
        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        container.add_css_class("terminal-pane");

        // Header container
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        header.add_css_class("terminal-pane-header");
        header.set_margin_start(6);
        header.set_margin_end(6);
        header.set_margin_top(2);
        header.set_margin_bottom(2);

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

        // Terminal widget
        let terminal = vte::Terminal::new();
        terminal.set_vexpand(true);
        terminal.set_hexpand(true);
        terminal.set_can_focus(true);
        terminal.set_scroll_on_output(true);
        terminal.set_scroll_on_keystroke(true);

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
        container.append(&terminal);

        let current_directory = Rc::new(RefCell::new(initial_directory.map(|p| p.to_path_buf())));
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
            let callbacks = Rc::clone(&close_callbacks);
            close_btn.connect_clicked(move |_| {
                if let Ok(list) = callbacks.try_borrow() {
                    for cb in list.iter() {
                        cb(pane_id);
                    }
                }
            });
        }

        // Wire split buttons
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

        // Wire child exit
        {
            let close_cbs = Rc::clone(&close_callbacks);
            let exit_cbs = Rc::clone(&child_exit_callbacks);
            let is_closing = Rc::clone(&is_closing);
            terminal.connect_child_exited(move |_term, status| {
                if is_closing.get() {
                    return;
                }
                if let Ok(exit_list) = exit_cbs.try_borrow() {
                    for cb in exit_list.iter() {
                        cb(pane_id, status);
                    }
                }
                if let Ok(list) = close_cbs.try_borrow() {
                    for cb in list.iter() {
                        cb(pane_id);
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

        // Wire OSC 7 current directory uri changes
        {
            let cwd_clone = Rc::clone(&current_directory);
            terminal.connect_current_directory_uri_changed(move |term| {
                if let Some(uri) = term.current_directory_uri() {
                    if let Some(path) = parse_osc7_uri(&uri) {
                        if let Ok(mut cwd) = cwd_clone.try_borrow_mut() {
                            *cwd = Some(path);
                        }
                    }
                }
            });
        }

        // Wire window title change
        {
            let label_clone = title_label.clone();
            let callbacks = Rc::clone(&title_callbacks);
            terminal.connect_window_title_changed(move |term| {
                if let Some(title) = term.window_title() {
                    label_clone.set_text(&title);
                    if let Ok(list) = callbacks.try_borrow() {
                        for cb in list.iter() {
                            cb(pane_id, &title);
                        }
                    }
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

        // Spawn shell asynchronously
        let shell = detect_shell();
        let env_vars = default_env();
        let env_refs: Vec<&str> = env_vars.iter().map(|s| s.as_str()).collect();
        let init_dir_str = initial_directory.and_then(|p| p.to_str());
        terminal.spawn_async(
            vte::PtyFlags::DEFAULT,
            init_dir_str,
            &[&shell],
            &env_refs,
            glib::SpawnFlags::DEFAULT,
            || {},
            -1,
            gio::Cancellable::NONE,
            |res| {
                if let Err(e) = res {
                    glib::g_warning!("Tilix", "Failed to spawn shell: {}", e);
                }
            },
        );

        Self {
            container,
            header,
            title_label,
            sync_btn,
            split_h_btn,
            split_v_btn,
            close_btn,
            terminal,
            pane_id,
            current_directory,
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
        self.container.upcast_ref()
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
        } else {
            self.container.remove_css_class("active-pane");
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

    pub fn apply_profile(&self, profile: &Profile) {
        self.apply_color_scheme(&profile.color_scheme);

        if let Some(ref font_name) = profile.font {
            let font_desc = gtk::pango::FontDescription::from_string(font_name);
            self.terminal.set_font(Some(&font_desc));
        }

        if let Some(lines) = profile.scrollback_lines {
            self.terminal.set_scrollback_lines(lines);
        }

        match profile.cursor_shape {
            CursorShapePreference::Block => {
                self.terminal.set_cursor_shape(vte::CursorShape::Block);
            }
            CursorShapePreference::IBeam => {
                self.terminal.set_cursor_shape(vte::CursorShape::Ibeam);
            }
            CursorShapePreference::Underline => {
                self.terminal.set_cursor_shape(vte::CursorShape::Underline);
            }
        }

        match profile.cursor_blink {
            CursorBlinkPreference::System => {
                self.terminal
                    .set_cursor_blink_mode(vte::CursorBlinkMode::System);
            }
            CursorBlinkPreference::On => {
                self.terminal.set_cursor_blink_mode(vte::CursorBlinkMode::On);
            }
            CursorBlinkPreference::Off => {
                self.terminal.set_cursor_blink_mode(vte::CursorBlinkMode::Off);
            }
        }
    }

    pub fn current_directory(&self) -> Option<PathBuf> {
        self.current_directory.borrow().clone()
    }

    pub fn connect_bell<F: Fn(PaneId) + 'static>(&self, f: F) {
        self.bell_callbacks.borrow_mut().push(Box::new(f));
    }

    pub fn connect_child_exited<F: Fn(PaneId, i32) + 'static>(&self, f: F) {
        self.child_exit_callbacks.borrow_mut().push(Box::new(f));
    }

    pub fn setup_drag_source(&self) {
        crate::ui::dnd::setup_pane_drag_source(&self.header, self.pane_id);
    }

    pub fn setup_drop_target<F: Fn(PaneId, PaneId) + 'static>(&self, on_swap: F) {
        crate::ui::dnd::setup_pane_drop_target(&self.container, self.pane_id, on_swap);
    }

    pub fn set_header_visible(&self, visible: bool) {
        self.header.set_visible(visible);
    }

    pub fn is_header_visible(&self) -> bool {
        self.header.get_visible()
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
}

