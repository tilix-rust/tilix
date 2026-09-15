use std::cell::RefCell;
use std::rc::Rc;

use gtk4 as gtk;
use gtk::prelude::*;
use vte4 as vte;
use vte::prelude::*;

use crate::model::{PaneId, SplitOrientation};
use crate::pty::{default_env, detect_shell};

type CloseCallback = Box<dyn Fn(PaneId)>;
type SplitCallback = Box<dyn Fn(PaneId, SplitOrientation)>;
type FocusCallback = Box<dyn Fn(PaneId)>;

#[derive(Clone)]
pub struct TerminalPane {
    container: gtk::Box,
    header: gtk::Box,
    title_label: gtk::Label,
    terminal: vte::Terminal,
    pane_id: PaneId,
    close_callbacks: Rc<RefCell<Vec<CloseCallback>>>,
    split_callbacks: Rc<RefCell<Vec<SplitCallback>>>,
    focus_callbacks: Rc<RefCell<Vec<FocusCallback>>>,
}

impl TerminalPane {
    pub fn new(pane_id: PaneId) -> Self {
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
        let split_h_btn = gtk::Button::from_icon_name("object-flip-horizontal-symbolic");
        split_h_btn.set_tooltip_text(Some("Split Right (Ctrl+Shift+R)"));
        split_h_btn.add_css_class("flat");

        let split_v_btn = gtk::Button::from_icon_name("object-flip-vertical-symbolic");
        split_v_btn.set_tooltip_text(Some("Split Down (Ctrl+Shift+D)"));
        split_v_btn.add_css_class("flat");

        let close_btn = gtk::Button::from_icon_name("window-close-symbolic");
        close_btn.set_tooltip_text(Some("Close Pane (Ctrl+Shift+W)"));
        close_btn.add_css_class("flat");

        header.append(&split_h_btn);
        header.append(&split_v_btn);
        header.append(&close_btn);

        // Terminal widget
        let terminal = vte::Terminal::new();
        terminal.set_vexpand(true);
        terminal.set_hexpand(true);
        terminal.set_can_focus(true);

        container.append(&header);
        container.append(&terminal);

        let close_callbacks: Rc<RefCell<Vec<CloseCallback>>> = Rc::new(RefCell::new(Vec::new()));
        let split_callbacks: Rc<RefCell<Vec<SplitCallback>>> = Rc::new(RefCell::new(Vec::new()));
        let focus_callbacks: Rc<RefCell<Vec<FocusCallback>>> = Rc::new(RefCell::new(Vec::new()));

        // Wire close button
        {
            let callbacks = Rc::clone(&close_callbacks);
            close_btn.connect_clicked(move |_| {
                let list = callbacks.borrow();
                for cb in list.iter() {
                    cb(pane_id);
                }
            });
        }

        // Wire split buttons
        {
            let callbacks = Rc::clone(&split_callbacks);
            split_h_btn.connect_clicked(move |_| {
                let list = callbacks.borrow();
                for cb in list.iter() {
                    cb(pane_id, SplitOrientation::Horizontal);
                }
            });
        }
        {
            let callbacks = Rc::clone(&split_callbacks);
            split_v_btn.connect_clicked(move |_| {
                let list = callbacks.borrow();
                for cb in list.iter() {
                    cb(pane_id, SplitOrientation::Vertical);
                }
            });
        }

        // Wire child exit
        {
            let callbacks = Rc::clone(&close_callbacks);
            terminal.connect_child_exited(move |_term, _status| {
                let list = callbacks.borrow();
                for cb in list.iter() {
                    cb(pane_id);
                }
            });
        }

        // Wire window title change
        {
            let label_clone = title_label.clone();
            terminal.connect_window_title_changed(move |term| {
                if let Some(title) = term.window_title() {
                    label_clone.set_text(&title);
                }
            });
        }

        // Wire focus notification
        {
            let callbacks = Rc::clone(&focus_callbacks);
            let focus_ctrl = gtk::EventControllerFocus::new();
            focus_ctrl.connect_enter(move |_| {
                let list = callbacks.borrow();
                for cb in list.iter() {
                    cb(pane_id);
                }
            });
            terminal.add_controller(focus_ctrl);
        }

        // Spawn shell asynchronously
        let shell = detect_shell();
        let env_vars = default_env();
        let env_refs: Vec<&str> = env_vars.iter().map(|s| s.as_str()).collect();
        terminal.spawn_async(
            vte::PtyFlags::DEFAULT,
            None,
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
            terminal,
            pane_id,
            close_callbacks,
            split_callbacks,
            focus_callbacks,
        }
    }

    pub fn pane_id(&self) -> PaneId {
        self.pane_id
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
}
