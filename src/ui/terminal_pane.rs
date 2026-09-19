use std::cell::{Cell, RefCell};
use std::os::unix::io::FromRawFd;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk4 as gtk;
use gtk::prelude::*;
use vte4 as vte;
use vte::prelude::*;

use crate::model::{
    ColorScheme, CursorBlinkPreference, CursorShapePreference, DockPosition,
    ExitActionPreference, PaneId, Profile, SplitOrientation,
};
use crate::pty::{
    default_env, detect_shell, parse_osc7_uri, Osc52Operation, Osc52Target, PtyProxy,
};

type CloseCallback = Box<dyn Fn(PaneId)>;
type SplitCallback = Box<dyn Fn(PaneId, SplitOrientation)>;
type FocusCallback = Box<dyn Fn(PaneId)>;
type CommitCallback = Box<dyn Fn(PaneId, &str)>;
type TitleCallback = Box<dyn Fn(PaneId, &str)>;
type SyncToggleCallback = Box<dyn Fn(PaneId, bool)>;
type BellCallback = Box<dyn Fn(PaneId)>;
type ChildExitCallback = Box<dyn Fn(PaneId, i32)>;
type DockCallback = Box<dyn Fn(PaneId, PaneId, DockPosition)>;

pub const ZOOM_STEP: f64 = 0.1;
pub const ZOOM_MIN: f64 = 0.2;
pub const ZOOM_MAX: f64 = 5.0;
pub const ZOOM_NORMAL: f64 = 1.0;

#[derive(Clone)]
struct PaneWidgets {
    terminal: vte::Terminal,
    scrollbar: gtk::Scrollbar,
    badge_label: gtk::Label,
    margin_line: gtk::Box,
    title_label: gtk::Label,
    raw_title: Rc<RefCell<String>>,
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
    raw_title: Rc<RefCell<String>>,
    sync_btn: gtk::ToggleButton,
    split_h_btn: gtk::Button,
    split_v_btn: gtk::Button,
    close_btn: gtk::Button,
    terminal: vte::Terminal,
    font_scale: Rc<Cell<f64>>,
    pane_id: PaneId,
    child_pid: Rc<Cell<Option<i32>>>,
    current_directory: Rc<RefCell<Option<PathBuf>>>,
    current_profile: Rc<RefCell<Profile>>,
    pty_proxy: Rc<RefCell<Option<std::sync::Arc<crate::pty::PtyProxy>>>>,
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

        let title_label = gtk::Label::new(None);
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

        let font_scale = Rc::new(Cell::new(ZOOM_NORMAL));

        // Wire scroll controller for font zoom interception (Ctrl + scroll)
        {
            let scroll_controller =
                gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
            scroll_controller.set_propagation_phase(gtk::PropagationPhase::Capture);
            let term_ref = terminal.clone();
            let font_scale_rc = Rc::clone(&font_scale);
            scroll_controller.connect_scroll(move |controller, _dx, dy| {
                let state = controller.current_event_state();
                let has_ctrl = state.contains(gtk::gdk::ModifierType::CONTROL_MASK);
                let has_shift = state.contains(gtk::gdk::ModifierType::SHIFT_MASK);
                let has_alt = state.contains(gtk::gdk::ModifierType::ALT_MASK);

                if has_ctrl && !has_shift && !has_alt {
                    if dy < 0.0 {
                        let current = font_scale_rc.get();
                        let next = (((current + ZOOM_STEP) * 10.0).round() / 10.0)
                            .clamp(ZOOM_MIN, ZOOM_MAX);
                        font_scale_rc.set(next);
                        term_ref.set_font_scale(next);
                        gtk::glib::Propagation::Stop
                    } else if dy > 0.0 {
                        let current = font_scale_rc.get();
                        let next = (((current - ZOOM_STEP) * 10.0).round() / 10.0)
                            .clamp(ZOOM_MIN, ZOOM_MAX);
                        font_scale_rc.set(next);
                        term_ref.set_font_scale(next);
                        gtk::glib::Propagation::Stop
                    } else {
                        gtk::glib::Propagation::Proceed
                    }
                } else {
                    gtk::glib::Propagation::Proceed
                }
            });
            terminal.add_controller(scroll_controller);
        }

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

        let effective_dir = initial_directory
            .map(|p| p.to_path_buf())
            .or_else(|| std::env::current_dir().ok());
        let current_directory = Rc::new(RefCell::new(effective_dir.clone()));
        let raw_title = Rc::new(RefCell::new("Terminal".to_string()));
        let current_profile = Rc::new(RefCell::new(Profile::default()));

        // Pre-configure VTE natural geometry to profile default size (e.g. 80x24)
        let default_cols = current_profile.borrow().default_size_columns.max(1) as i64;
        let default_rows = current_profile.borrow().default_size_rows.max(1) as i64;
        terminal.set_size(default_cols, default_rows);

        let initial_formatted_title = Self::format_pane_title(
            &current_profile.borrow(),
            pane_id,
            "Terminal",
            effective_dir.as_deref(),
            &terminal,
            None,
            true,
        );
        title_label.set_text(&initial_formatted_title);

        let pane_widgets = PaneWidgets {
            terminal: terminal.clone(),
            scrollbar: scrollbar.clone(),
            badge_label: badge_label.clone(),
            margin_line: margin_line.clone(),
            title_label: title_label.clone(),
            raw_title: Rc::clone(&raw_title),
        };

        let child_pid: Rc<Cell<Option<i32>>> = Rc::new(Cell::new(None));
        let pty_proxy: Rc<RefCell<Option<std::sync::Arc<PtyProxy>>>> = Rc::new(RefCell::new(None));
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
            let pty_proxy_exit = Rc::clone(&pty_proxy);

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
                        is_closing.set(true);
                        if let Ok(list) = close_cbs.try_borrow() {
                            for cb in list.iter() {
                                cb(pane_id);
                            }
                        }
                    }
                    ExitActionPreference::Restart => {
                        let dir = current_dir.borrow().clone();
                        Self::spawn_shell_process(
                            term,
                            &profile_rc.borrow(),
                            dir.as_deref(),
                            &pid_cell,
                            Some(&pty_proxy_exit),
                            None,
                        );
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
            let pid_clone = Rc::clone(&child_pid);
            let is_sync_clone = Rc::clone(&is_sync_enabled);
            let widgets = pane_widgets.clone();
            let callbacks = Rc::clone(&title_callbacks);
            terminal.connect_current_directory_uri_changed(move |term| {
                if let Some(uri) = term.current_directory_uri() {
                    if let Some(path) = parse_osc7_uri(&uri) {
                        if let Ok(mut cwd) = cwd_clone.try_borrow_mut() {
                            *cwd = Some(path.clone());
                        }
                        Self::refresh_title_static(
                            &widgets,
                            &prof_rc,
                            &cwd_clone,
                            &pid_clone,
                            &is_sync_clone,
                            pane_id,
                            &callbacks,
                        );
                        Self::check_auto_switch_static(
                            &prof_rc,
                            Some(&path),
                            pane_id,
                            &widgets,
                            pid_clone.get(),
                            is_sync_clone.get(),
                        );
                    }
                }
            });
        }

        // Wire window title change and automatic switch evaluation
        {
            let callbacks = Rc::clone(&title_callbacks);
            let prof_rc = Rc::clone(&current_profile);
            let cwd_clone = Rc::clone(&current_directory);
            let pid_clone = Rc::clone(&child_pid);
            let is_sync_clone = Rc::clone(&is_sync_enabled);
            let widgets = pane_widgets.clone();
            terminal.connect_window_title_changed(move |term| {
                if let Some(title) = term.window_title() {
                    *widgets.raw_title.borrow_mut() = title.to_string();
                }
                Self::refresh_title_static(
                    &widgets,
                    &prof_rc,
                    &cwd_clone,
                    &pid_clone,
                    &is_sync_clone,
                    pane_id,
                    &callbacks,
                );
                let dir = cwd_clone.borrow().clone();
                Self::check_auto_switch_static(
                    &prof_rc,
                    dir.as_deref(),
                    pane_id,
                    &widgets,
                    pid_clone.get(),
                    is_sync_clone.get(),
                );
            });
        }

        // Wire focus notification
        {
            let callbacks = Rc::clone(&focus_callbacks);
            let prof_rc = Rc::clone(&current_profile);
            let cwd_clone = Rc::clone(&current_directory);
            let pid_clone = Rc::clone(&child_pid);
            let is_sync_clone = Rc::clone(&is_sync_enabled);
            let title_cbs = Rc::clone(&title_callbacks);
            let widgets = pane_widgets.clone();
            let focus_ctrl = gtk::EventControllerFocus::new();
            focus_ctrl.connect_enter(move |_| {
                Self::refresh_title_static(
                    &widgets,
                    &prof_rc,
                    &cwd_clone,
                    &pid_clone,
                    &is_sync_clone,
                    pane_id,
                    &title_cbs,
                );
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

        // Setup terminal context menu
        let menu = gio::Menu::new();

        // Section 1: Clipboard
        let clip_section = gio::Menu::new();
        clip_section.append(Some("Copy"), Some("win.copy"));
        clip_section.append(Some("Copy as HTML"), Some("win.copy-html"));
        clip_section.append(Some("Paste"), Some("win.paste"));
        clip_section.append(Some("Paste Primary Selection"), Some("win.paste-primary"));
        menu.append_section(None, &clip_section);

        // Section 2: Selection
        let sel_section = gio::Menu::new();
        sel_section.append(Some("Select All"), Some("win.select-all"));
        menu.append_section(None, &sel_section);

        // Section 3: Splits
        let split_section = gio::Menu::new();
        split_section.append(Some("Split Right"), Some("win.split-right"));
        split_section.append(Some("Split Down"), Some("win.split-down"));
        split_section.append(Some("Close Terminal"), Some("win.close-pane"));
        menu.append_section(None, &split_section);

        // Section 4: Preferences
        let pref_section = gio::Menu::new();
        pref_section.append(Some("Preferences..."), Some("win.preferences"));
        menu.append_section(None, &pref_section);

        terminal.set_context_menu_model(Some(&menu));

        // Connect copy-on-select
        let profile_copy_select = Rc::clone(&current_profile);
        terminal.connect_selection_changed(move |term| {
            if profile_copy_select.borrow().copy_on_select && term.has_selection() {
                term.copy_clipboard_format(vte::Format::Text);
            }
        });

        // Synchronize terminal window size to PTY proxy on dimension / font changes
        {
            let pty_proxy_size = Rc::clone(&pty_proxy);
            let update_size = Rc::new(move |term: &vte::Terminal| {
                if let Some(ref proxy) = *pty_proxy_size.borrow() {
                    let rows = term.row_count();
                    let cols = term.column_count();
                    let r = if rows > 0 { rows as u16 } else { 24 };
                    let c = if cols > 0 { cols as u16 } else { 80 };
                    proxy.set_window_size(r, c);
                }
            });

            let u1 = Rc::clone(&update_size);
            terminal.connect_char_size_changed(move |term, _w, _h| {
                u1(term);
            });

            let u2 = Rc::clone(&update_size);
            terminal.connect_resize_window(move |term, _w, _h| {
                u2(term);
            });

            // Frame-synchronous size sync: ensures initial allocation and runtime resizing
            // immediately propagate to PTY proxy inner master before shell prompt draws.
            let pty_proxy_tick = Rc::clone(&pty_proxy);
            let last_cols = Cell::new(0i64);
            let last_rows = Cell::new(0i64);
            terminal.add_tick_callback(move |term, _clock| {
                let cols = term.column_count();
                let rows = term.row_count();
                if cols > 0 && rows > 0 && (cols != last_cols.get() || rows != last_rows.get()) {
                    last_cols.set(cols);
                    last_rows.set(rows);
                    if let Some(ref proxy) = *pty_proxy_tick.borrow() {
                        proxy.set_window_size(rows as u16, cols as u16);
                    }
                }
                glib::ControlFlow::Continue
            });
        }

        // Spawn initial shell process:
        // If terminal dimensions are already allocated (e.g. split pane or existing window), spawn immediately.
        // If not yet allocated (first pane of a new window), wait for the first render frame (add_tick_callback)
        // so the PTY proxy and child shell initialize with exact window dimensions (avoiding zsh prompt race and '%').
        {
            let prof_rc = Rc::clone(&current_profile);
            let cwd_clone = Rc::clone(&current_directory);
            let pid_clone = Rc::clone(&child_pid);
            let is_sync_clone = Rc::clone(&is_sync_enabled);
            let title_cbs = Rc::clone(&title_callbacks);
            let widgets = pane_widgets.clone();
            let term_for_spawn = terminal.clone();
            let pty_proxy_spawn = Rc::clone(&pty_proxy);
            let spawned_flag = Rc::new(Cell::new(false));

            let do_spawn = {
                let spawned_flag = Rc::clone(&spawned_flag);
                Rc::new(move || {
                    if spawned_flag.get() {
                        return;
                    }
                    spawned_flag.set(true);

                    let prof_rc = Rc::clone(&prof_rc);
                    let cwd_clone = Rc::clone(&cwd_clone);
                    let pid_clone = Rc::clone(&pid_clone);
                    let is_sync_clone = Rc::clone(&is_sync_clone);
                    let title_cbs = Rc::clone(&title_cbs);
                    let widgets = widgets.clone();

                    let prof_for_cb = Rc::clone(&prof_rc);
                    let pid_for_cb = Rc::clone(&pid_clone);
                    let prof_snapshot = prof_rc.borrow().clone();

                    Self::spawn_shell_process(
                        &term_for_spawn,
                        &prof_snapshot,
                        effective_dir.as_deref(),
                        &pid_clone,
                        Some(&pty_proxy_spawn),
                        Some(Box::new(move |pid| {
                            Self::refresh_title_static(
                                &widgets,
                                &prof_for_cb,
                                &cwd_clone,
                                &pid_for_cb,
                                &is_sync_clone,
                                pane_id,
                                &title_cbs,
                            );
                            let dir = cwd_clone.borrow().clone();
                            Self::check_auto_switch_static(
                                &prof_for_cb,
                                dir.as_deref(),
                                pane_id,
                                &widgets,
                                Some(pid),
                                is_sync_clone.get(),
                            );
                        })),
                    );
                })
            };

            let has_allocated_geometry = terminal.is_mapped()
                && terminal.width() > 0
                && terminal.column_count() > 0
                && terminal.row_count() > 0;

            if has_allocated_geometry {
                do_spawn();
            } else {
                let do_spawn_tick = Rc::clone(&do_spawn);
                terminal.add_tick_callback(move |term, _clock| {
                    if term.is_mapped() && term.width() > 0 && term.column_count() > 0 && term.row_count() > 0 {
                        do_spawn_tick();
                        glib::ControlFlow::Break
                    } else {
                        glib::ControlFlow::Continue
                    }
                });

                let do_spawn_map = Rc::clone(&do_spawn);
                terminal.connect_map(move |term| {
                    if term.width() > 0 && term.column_count() > 0 && term.row_count() > 0 {
                        do_spawn_map();
                    }
                });

                // Fallback for headless testing environments or unmapped offscreen widgets
                let do_spawn_fallback = do_spawn;
                glib::timeout_add_local_once(std::time::Duration::from_millis(150), move || {
                    do_spawn_fallback();
                });
            }
        }

        Self {
            overlay,
            drop_indicator,
            badge_label,
            margin_line,
            scrollbar,
            container,
            header,
            title_label,
            raw_title,
            sync_btn,
            split_h_btn,
            split_v_btn,
            close_btn,
            terminal,
            font_scale,
            pane_id,
            child_pid,
            current_directory,
            current_profile,
            pty_proxy,
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

    #[cfg(target_os = "linux")]
    fn get_foreground_pid_linux(shell_pid: i32) -> Option<i32> {
        let stat = std::fs::read_to_string(format!("/proc/{}/stat", shell_pid)).ok()?;
        let rparen = stat.rfind(')')?;
        let rest = &stat[rparen + 1..];
        let mut parts = rest.split_whitespace();
        let tpgid_str = parts.nth(5)?;
        let tpgid: i32 = tpgid_str.parse().ok()?;
        if tpgid > 0 {
            Some(tpgid)
        } else {
            None
        }
    }

    #[cfg(target_os = "linux")]
    fn get_foreground_cwd_linux(shell_pid: i32) -> Option<PathBuf> {
        if let Some(fg_pid) = Self::get_foreground_pid_linux(shell_pid) {
            if let Ok(target) = std::fs::read_link(format!("/proc/{}/cwd", fg_pid)) {
                return Some(target);
            }
        }
        None
    }

    #[cfg(target_os = "linux")]
    fn get_foreground_comm_linux(shell_pid: i32) -> Option<String> {
        let target_pid = Self::get_foreground_pid_linux(shell_pid).unwrap_or(shell_pid);
        std::fs::read_to_string(format!("/proc/{}/comm", target_pid))
            .ok()
            .map(|s| s.trim().to_string())
    }

    fn resolve_directory(
        cached_dir: &Rc<RefCell<Option<PathBuf>>>,
        child_pid: Option<i32>,
    ) -> Option<PathBuf> {
        #[cfg(target_os = "linux")]
        if let Some(pid) = child_pid {
            if let Some(fg_cwd) = Self::get_foreground_cwd_linux(pid) {
                if let Ok(mut c) = cached_dir.try_borrow_mut() {
                    *c = Some(fg_cwd.clone());
                }
                return Some(fg_cwd);
            }
            if let Ok(target) = std::fs::read_link(format!("/proc/{}/cwd", pid)) {
                if let Ok(mut c) = cached_dir.try_borrow_mut() {
                    *c = Some(target.clone());
                }
                return Some(target);
            }
        }
        if let Ok(c) = cached_dir.try_borrow() {
            if let Some(ref dir) = *c {
                return Some(dir.clone());
            }
        }
        std::env::current_dir().ok()
    }

    fn format_pane_title(
        profile: &Profile,
        pane_id: PaneId,
        raw_title: &str,
        directory: Option<&Path>,
        terminal: &vte::Terminal,
        child_pid: Option<i32>,
        is_sync: bool,
    ) -> String {
        let mut token_ctx = crate::model::title::TokenContext::new_terminal(raw_title);
        token_ctx.id = Some(pane_id.0);
        token_ctx.profile_name = Some(profile.name.clone());
        token_ctx.directory = directory.map(|d| d.to_path_buf());
        token_ctx.app_name = Some("Tilix".to_string());
        let cols = terminal.column_count();
        let rows = terminal.row_count();
        if cols > 0 {
            token_ctx.columns = Some(cols as u32);
        }
        if rows > 0 {
            token_ctx.rows = Some(rows as u32);
        }
        token_ctx.input_sync = is_sync;
        #[cfg(target_os = "linux")]
        if let Some(pid) = child_pid {
            if let Some(comm) = Self::get_foreground_comm_linux(pid) {
                token_ctx.process = Some(comm);
            }
        }
        crate::model::title::expand_title_tokens_scoped(
            &profile.terminal_title,
            crate::model::title::TitleEditScope::Terminal,
            &token_ctx,
        )
    }

    fn refresh_title_static(
        widgets: &PaneWidgets,
        profile_rc: &Rc<RefCell<Profile>>,
        cwd_rc: &Rc<RefCell<Option<PathBuf>>>,
        pid_cell: &Rc<Cell<Option<i32>>>,
        is_sync_cell: &Rc<Cell<bool>>,
        pane_id: PaneId,
        callbacks: &Rc<RefCell<Vec<TitleCallback>>>,
    ) -> String {
        let raw = widgets.raw_title.borrow().clone();
        let dir = Self::resolve_directory(cwd_rc, pid_cell.get());
        let prof = profile_rc.borrow().clone();
        let expanded = Self::format_pane_title(
            &prof,
            pane_id,
            &raw,
            dir.as_deref(),
            &widgets.terminal,
            pid_cell.get(),
            is_sync_cell.get(),
        );
        widgets.title_label.set_text(&expanded);
        if let Ok(list) = callbacks.try_borrow() {
            for cb in list.iter() {
                cb(pane_id, &expanded);
            }
        }
        expanded
    }

    fn spawn_shell_process(
        terminal: &vte::Terminal,
        profile: &Profile,
        directory: Option<&Path>,
        child_pid: &Rc<Cell<Option<i32>>>,
        pty_proxy_slot: Option<&Rc<RefCell<Option<std::sync::Arc<crate::pty::PtyProxy>>>>>,
        on_spawned: Option<Box<dyn Fn(i32)>>,
    ) {
        let shell = detect_shell();
        let env_vars = default_env();
        let env_refs: Vec<&str> = env_vars.iter().map(|s| s.as_str()).collect();
        let init_dir_str = directory.and_then(|p| p.to_str());
        let (_cmd, argv) = crate::pty::shell::build_spawn_args(profile, &shell);
        let argv_refs: Vec<&str> = argv.iter().map(|s| s.as_str()).collect();
        let pid_cell = Rc::clone(child_pid);

        if profile.enable_osc52 {
            match PtyProxy::new() {
                Ok(proxy) => {
                    let proxy = std::sync::Arc::new(proxy);

                    // Initialize terminal window size on proxy (defaults to profile configured size if 0)
                    let rows = terminal.row_count();
                    let cols = terminal.column_count();
                    let r = if rows > 0 { rows as u16 } else { profile.default_size_rows.max(24) as u16 };
                    let c = if cols > 0 { cols as u16 } else { profile.default_size_columns.max(80) as u16 };
                    proxy.set_window_size(r, c);

                    let om_fd = match proxy.take_outer_master() {
                        Some(fd) => fd,
                        None => {
                            glib::g_warning!("Tilix", "Outer master FD unavailable in PtyProxy");
                            return;
                        }
                    };
                    let owned = unsafe { std::os::unix::io::OwnedFd::from_raw_fd(om_fd) };
                    match vte::Pty::foreign_sync(owned, gio::Cancellable::NONE) {
                        Ok(vte_pty) => {
                            let _ = vte_pty.set_size(r as i32, c as i32);
                            terminal.set_pty(Some(&vte_pty));

                            let allow_query = profile.osc52_allow_query;
                            let proxy_clone = std::sync::Arc::clone(&proxy);
                            proxy.start(
                                move |event| {
                                    glib::idle_add_once(move || {
                                        let display = gtk::gdk::Display::default();
                                        match event.operation {
                                            Osc52Operation::Write(ref data) => {
                                                if let Ok(text) = std::str::from_utf8(data) {
                                                    for target in &event.targets {
                                                        match target {
                                                            Osc52Target::Clipboard => {
                                                                if let Some(disp) = &display {
                                                                    disp.clipboard().set_text(text);
                                                                }
                                                            }
                                                            Osc52Target::Primary => {
                                                                if let Some(disp) = &display {
                                                                    disp.primary_clipboard().set_text(text);
                                                                }
                                                            }
                                                            _ => {
                                                                if let Some(disp) = &display {
                                                                    disp.clipboard().set_text(text);
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            Osc52Operation::Clear => {
                                                for target in &event.targets {
                                                    match target {
                                                        Osc52Target::Clipboard => {
                                                            if let Some(disp) = &display {
                                                                disp.clipboard().set_text("");
                                                            }
                                                        }
                                                        Osc52Target::Primary => {
                                                            if let Some(disp) = &display {
                                                                disp.primary_clipboard().set_text("");
                                                            }
                                                        }
                                                        _ => {}
                                                    }
                                                }
                                            }
                                            Osc52Operation::Query => {}
                                        }
                                    });
                                },
                                move |_targets| {
                                    if !allow_query {
                                        return None;
                                    }
                                    // Synchronous querying across threads is disabled for security and thread-safety
                                    None
                                },
                            );

                            use std::os::unix::process::CommandExt;
                            use std::process::Stdio;

                            let inner_slave = proxy.inner_slave_fd();
                            let stdin_fd = unsafe { libc::dup(inner_slave) };
                            let stdout_fd = unsafe { libc::dup(inner_slave) };
                            let stderr_fd = unsafe { libc::dup(inner_slave) };

                            let cmd_path = &argv[0];
                            let mut cmd = std::process::Command::new(cmd_path);
                            if argv.len() > 1 {
                                cmd.args(&argv[1..]);
                            }
                            if let Some(dir) = directory {
                                cmd.current_dir(dir);
                            }
                            for env_var in &env_vars {
                                if let Some((k, v)) = env_var.split_once('=') {
                                    cmd.env(k, v);
                                }
                            }
                            cmd.stdin(unsafe { Stdio::from_raw_fd(stdin_fd) });
                            cmd.stdout(unsafe { Stdio::from_raw_fd(stdout_fd) });
                            cmd.stderr(unsafe { Stdio::from_raw_fd(stderr_fd) });

                            unsafe {
                                cmd.pre_exec(move || {
                                    #[cfg(target_os = "linux")]
                                    libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
                                    libc::setsid();
                                    libc::ioctl(0, libc::TIOCSCTTY, 1);
                                    Ok(())
                                });
                            }

                            match cmd.spawn() {
                                Ok(child) => {
                                    // Close parent process copy of inner_slave so inner_master gets EOF when child exits
                                    proxy.close_inner_slave();

                                    let pid = child.id() as i32;
                                    pid_cell.set(Some(pid));
                                    terminal.watch_child(glib::Pid(pid));
                                    if let Some(slot) = pty_proxy_slot {
                                        *slot.borrow_mut() = Some(proxy_clone);
                                    }
                                    if let Some(ref cb) = on_spawned {
                                        cb(pid);
                                    }
                                    return;
                                }
                                Err(e) => {
                                    proxy.close_inner_slave();
                                    glib::g_warning!("Tilix", "Failed to spawn child with PTY proxy: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            glib::g_warning!("Tilix", "Failed to create foreign PTY: {}", e);
                        }
                    }
                }
                Err(e) => {
                    glib::g_warning!("Tilix", "Failed to create PTY proxy: {}", e);
                }
            }
        }

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
                        if let Some(ref cb) = on_spawned {
                            cb(pid.0);
                        }
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
        child_pid: Option<i32>,
        is_sync: bool,
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
                    let raw = widgets.raw_title.borrow().clone();
                    Self::apply_profile_to_widgets(
                        target,
                        pane_id,
                        &raw,
                        directory,
                        widgets,
                        child_pid,
                        is_sync,
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
        *self.raw_title.borrow_mut() = title.to_string();
        self.refresh_title();
    }

    pub fn raw_title(&self) -> String {
        self.raw_title.borrow().clone()
    }

    pub fn refresh_title(&self) -> String {
        let widgets = PaneWidgets {
            terminal: self.terminal.clone(),
            scrollbar: self.scrollbar.clone(),
            badge_label: self.badge_label.clone(),
            margin_line: self.margin_line.clone(),
            title_label: self.title_label.clone(),
            raw_title: Rc::clone(&self.raw_title),
        };
        Self::refresh_title_static(
            &widgets,
            &self.current_profile,
            &self.current_directory,
            &self.child_pid,
            &self.is_sync_enabled,
            self.pane_id,
            &self.title_callbacks,
        )
    }

    pub fn font_scale(&self) -> f64 {
        self.font_scale.get()
    }

    pub fn zoom_in(&self) {
        let current = self.font_scale.get();
        let next = (((current + ZOOM_STEP) * 10.0).round() / 10.0).clamp(ZOOM_MIN, ZOOM_MAX);
        self.font_scale.set(next);
        self.terminal.set_font_scale(next);
    }

    pub fn zoom_out(&self) {
        let current = self.font_scale.get();
        let next = (((current - ZOOM_STEP) * 10.0).round() / 10.0).clamp(ZOOM_MIN, ZOOM_MAX);
        self.font_scale.set(next);
        self.terminal.set_font_scale(next);
    }

    pub fn zoom_normal(&self) {
        self.font_scale.set(ZOOM_NORMAL);
        self.terminal.set_font_scale(ZOOM_NORMAL);
    }

    pub fn copy_clipboard(&self) {
        if !self.terminal.is_realized() {
            return;
        }
        self.terminal.copy_clipboard_format(vte4::Format::Text);
    }

    pub fn copy_html(&self) {
        if !self.terminal.is_realized() {
            return;
        }
        let html_opt = self.terminal.text_selected(vte4::Format::Html);
        let plain_opt = self.terminal.text_selected(vte4::Format::Text);

        if let (Some(html_str), Some(plain_str)) = (html_opt, plain_opt) {
            if let Some(display) = gtk::gdk::Display::default() {
                let html_bytes = glib::Bytes::from(html_str.as_bytes());
                let plain_bytes = glib::Bytes::from(plain_str.as_bytes());
                let html_provider = gtk::gdk::ContentProvider::for_bytes("text/html", &html_bytes);
                let plain_provider =
                    gtk::gdk::ContentProvider::for_bytes("text/plain;charset=utf-8", &plain_bytes);
                let text_provider = gtk::gdk::ContentProvider::for_bytes("text/plain", &plain_bytes);
                let union_provider = gtk::gdk::ContentProvider::new_union(&[
                    html_provider,
                    plain_provider,
                    text_provider,
                ]);
                let _ = display.clipboard().set_content(Some(&union_provider));
                return;
            }
        }
        self.terminal.copy_clipboard_format(vte4::Format::Html);
    }

    pub fn paste_clipboard(&self) {
        if !self.terminal.is_realized() {
            return;
        }
        self.terminal.paste_clipboard();
    }

    pub fn paste_primary(&self) {
        if !self.terminal.is_realized() {
            return;
        }
        self.terminal.paste_primary();
    }

    pub fn select_all(&self) {
        if !self.terminal.is_realized() {
            return;
        }
        self.terminal.select_all();
    }

    pub fn context_menu_model(&self) -> Option<gio::MenuModel> {
        self.terminal.context_menu_model()
    }

    pub fn pty_proxy(&self) -> Option<std::sync::Arc<crate::pty::PtyProxy>> {
        self.pty_proxy.borrow().clone()
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
        raw_title: &str,
        directory: Option<&Path>,
        widgets: &PaneWidgets,
        child_pid: Option<i32>,
        is_sync: bool,
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

        // Title format
        let expanded_title = Self::format_pane_title(
            profile,
            pane_id,
            raw_title,
            directory,
            &widgets.terminal,
            child_pid,
            is_sync,
        );
        widgets.title_label.set_text(&expanded_title);

        // Badge overlay
        if profile.badge_text.trim().is_empty() {
            widgets.badge_label.set_visible(false);
        } else {
            let mut token_ctx = crate::model::title::TokenContext::new_terminal(raw_title);
            token_ctx.id = Some(pane_id.0);
            token_ctx.profile_name = Some(profile.name.clone());
            token_ctx.directory = directory.map(|d| d.to_path_buf());
            token_ctx.app_name = Some("Tilix".to_string());
            let cols = widgets.terminal.column_count();
            let rows = widgets.terminal.row_count();
            if cols > 0 {
                token_ctx.columns = Some(cols as u32);
            }
            if rows > 0 {
                token_ctx.rows = Some(rows as u32);
            }
            token_ctx.input_sync = is_sync;
            #[cfg(target_os = "linux")]
            if let Some(pid) = child_pid {
                if let Some(comm) = Self::get_foreground_comm_linux(pid) {
                    token_ctx.process = Some(comm);
                }
            }
            let expanded_badge = crate::model::title::expand_title_tokens_scoped(
                &profile.badge_text,
                crate::model::title::TitleEditScope::Terminal,
                &token_ctx,
            );
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
        let default_cols = profile.default_size_columns.max(1) as i64;
        let default_rows = profile.default_size_rows.max(1) as i64;
        self.terminal.set_size(default_cols, default_rows);
        let dir = self.current_directory();
        let widgets = PaneWidgets {
            terminal: self.terminal.clone(),
            scrollbar: self.scrollbar.clone(),
            badge_label: self.badge_label.clone(),
            margin_line: self.margin_line.clone(),
            title_label: self.title_label.clone(),
            raw_title: Rc::clone(&self.raw_title),
        };
        let raw = self.raw_title.borrow().clone();
        Self::apply_profile_to_widgets(
            profile,
            self.pane_id,
            &raw,
            dir.as_deref(),
            &widgets,
            self.child_pid.get(),
            self.is_sync_enabled.get(),
        );
        let expanded = self.title_label.text().to_string();
        if let Ok(list) = self.title_callbacks.try_borrow() {
            for cb in list.iter() {
                cb(self.pane_id, &expanded);
            }
        }
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
        Self::resolve_directory(&self.current_directory, self.child_pid.get())
    }

    pub fn set_current_directory(&self, dir: Option<PathBuf>) {
        *self.current_directory.borrow_mut() = dir;
        self.refresh_title();
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
        crate::ui::dnd::setup_pane_drag_source(&self.header, self.clone(), false);
        crate::ui::dnd::setup_pane_drag_source(&self.terminal, self.clone(), true);
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

impl Drop for TerminalPane {
    fn drop(&mut self) {
        if let Some(pid) = self.child_pid.get() {
            unsafe {
                libc::kill(pid, libc::SIGTERM);
                libc::kill(pid, libc::SIGKILL);
                libc::waitpid(pid, std::ptr::null_mut(), libc::WNOHANG);
            }
        }
        if let Some(proxy) = self.pty_proxy.borrow_mut().take() {
            proxy.shutdown();
        }
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

    #[test]
    fn test_terminal_pane_zoom_operations() {
        crate::ui::window::run_gtk_test(|| {
            let pane = TerminalPane::new(PaneId(1), None);
            assert_eq!(pane.font_scale(), ZOOM_NORMAL);

            pane.zoom_in();
            assert!((pane.font_scale() - 1.1).abs() < 1e-6);

            pane.zoom_out();
            assert!((pane.font_scale() - 1.0).abs() < 1e-6);

            pane.zoom_normal();
            assert_eq!(pane.font_scale(), ZOOM_NORMAL);

            for _ in 0..20 {
                pane.zoom_out();
            }
            assert_eq!(pane.font_scale(), ZOOM_MIN);

            for _ in 0..60 {
                pane.zoom_in();
            }
            assert_eq!(pane.font_scale(), ZOOM_MAX);
        });
    }
}
