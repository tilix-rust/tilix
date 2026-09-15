use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gio::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;

use crate::model::{Direction, SplitOrientation};
use crate::ui::session_view::{SessionAction, SessionView};

pub struct TilixWindow {
    window: adw::ApplicationWindow,
    session_view: Rc<RefCell<SessionView>>,
}

pub fn setup_css() {
    let css_provider = gtk::CssProvider::new();
    css_provider.load_from_string(
        "
        .terminal-pane {
            border: 2px solid transparent;
            border-radius: 4px;
        }
        .terminal-pane.active-pane {
            border: 2px solid @accent_color;
        }
        .terminal-pane-header {
            background-color: alpha(@window_bg_color, 0.7);
            border-bottom: 1px solid alpha(@borders, 0.4);
            padding: 2px 4px;
        }
        ",
    );
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &css_provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

pub fn setup_accels(app: &adw::Application) {
    app.set_accels_for_action("win.split-right", &["<Primary><Shift>r"]);
    app.set_accels_for_action("win.split-down", &["<Primary><Shift>d"]);
    app.set_accels_for_action("win.close-pane", &["<Primary><Shift>w"]);
    app.set_accels_for_action("win.balance-layout", &["<Primary><Shift>b"]);
    app.set_accels_for_action("win.focus-up", &["<Alt>Up"]);
    app.set_accels_for_action("win.focus-down", &["<Alt>Down"]);
    app.set_accels_for_action("win.focus-left", &["<Alt>Left"]);
    app.set_accels_for_action("win.focus-right", &["<Alt>Right"]);
}

impl TilixWindow {
    pub fn new(app: &adw::Application) -> Self {
        let window = adw::ApplicationWindow::new(app);
        window.set_default_size(900, 600);
        window.set_title(Some("Tilix"));

        let header_bar = adw::HeaderBar::new();
        let title_widget = adw::WindowTitle::new("Tilix", "");
        header_bar.set_title_widget(Some(&title_widget));

        let split_h_btn = gtk::Button::from_icon_name("object-flip-horizontal-symbolic");
        split_h_btn.set_tooltip_text(Some("Split Right (Ctrl+Shift+R)"));
        split_h_btn.set_action_name(Some("win.split-right"));
        header_bar.pack_start(&split_h_btn);

        let split_v_btn = gtk::Button::from_icon_name("object-flip-vertical-symbolic");
        split_v_btn.set_tooltip_text(Some("Split Down (Ctrl+Shift+D)"));
        split_v_btn.set_action_name(Some("win.split-down"));
        header_bar.pack_start(&split_v_btn);

        let close_btn = gtk::Button::from_icon_name("window-close-symbolic");
        close_btn.set_tooltip_text(Some("Close Pane (Ctrl+Shift+W)"));
        close_btn.set_action_name(Some("win.close-pane"));
        header_bar.pack_end(&close_btn);

        let session_view = Rc::new(RefCell::new(SessionView::new()));

        // Wire session view action handler
        let session_weak = Rc::downgrade(&session_view);
        let win_weak = window.downgrade();
        session_view.borrow().set_action_handler(move |action| {
            let s_weak = session_weak.clone();
            let w_weak = win_weak.clone();
            glib::idle_add_local_once(move || {
                let Some(session) = s_weak.upgrade() else { return; };
                match action {
                    SessionAction::Split(id, orientation) => {
                        session.borrow_mut().set_active_pane(id);
                        session.borrow_mut().split_active(orientation);
                    }
                    SessionAction::Close(id) => {
                        session.borrow_mut().close_pane(id);
                        if session.borrow().is_empty() {
                            if let Some(win) = w_weak.upgrade() {
                                win.close();
                            }
                        }
                    }
                    SessionAction::Focus(id) => {
                        session.borrow_mut().set_active_pane(id);
                    }
                }
            });
        });

        // Set content inside toolbar view
        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header_bar);
        toolbar_view.set_content(Some(session_view.borrow().widget()));
        window.set_content(Some(&toolbar_view));

        // Register window actions
        let tilix_win = Self {
            window,
            session_view,
        };
        tilix_win.setup_actions();

        tilix_win
    }

    fn setup_actions(&self) {
        // Split Right
        {
            let action = gio::SimpleAction::new("split-right", None);
            let sv = Rc::clone(&self.session_view);
            action.connect_activate(move |_, _| {
                sv.borrow_mut().split_active(SplitOrientation::Horizontal);
            });
            self.window.add_action(&action);
        }

        // Split Down
        {
            let action = gio::SimpleAction::new("split-down", None);
            let sv = Rc::clone(&self.session_view);
            action.connect_activate(move |_, _| {
                sv.borrow_mut().split_active(SplitOrientation::Vertical);
            });
            self.window.add_action(&action);
        }

        // Close Pane
        {
            let action = gio::SimpleAction::new("close-pane", None);
            let sv = Rc::clone(&self.session_view);
            let win_weak = self.window.downgrade();
            action.connect_activate(move |_, _| {
                sv.borrow_mut().close_active();
                if sv.borrow().is_empty() {
                    if let Some(win) = win_weak.upgrade() {
                        win.close();
                    }
                }
            });
            self.window.add_action(&action);
        }

        // Balance Layout
        {
            let action = gio::SimpleAction::new("balance-layout", None);
            let sv = Rc::clone(&self.session_view);
            action.connect_activate(move |_, _| {
                sv.borrow_mut().balance_layout();
            });
            self.window.add_action(&action);
        }

        // Directional focus
        let directions = [
            ("focus-up", Direction::Up),
            ("focus-down", Direction::Down),
            ("focus-left", Direction::Left),
            ("focus-right", Direction::Right),
        ];

        for (name, dir) in directions {
            let action = gio::SimpleAction::new(name, None);
            let sv = Rc::clone(&self.session_view);
            action.connect_activate(move |_, _| {
                sv.borrow_mut().focus_adjacent(dir);
            });
            self.window.add_action(&action);
        }
    }

    pub fn window(&self) -> &adw::ApplicationWindow {
        &self.window
    }

    pub fn session_view(&self) -> &Rc<RefCell<SessionView>> {
        &self.session_view
    }

    pub fn present(&self) {
        self.window.present();
    }
}
