use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;

use crate::ui::session_view::{SessionAction, SessionView};

#[derive(Clone)]
pub struct TilixQuakeWindow {
    window: adw::ApplicationWindow,
    session_view: Rc<RefCell<SessionView>>,
}

impl TilixQuakeWindow {
    pub fn new(app: &adw::Application) -> Self {
        let window = adw::ApplicationWindow::new(app);
        window.set_title(Some("Tilix Quake"));
        window.add_css_class("quake-window");
        window.set_valign(gtk::Align::Start);
        window.set_default_size(1200, 400);

        let session_view = Rc::new(RefCell::new(SessionView::new()));
        if session_view.borrow().has_transparent_pane() {
            window.add_css_class("transparent-window");
        }
        window.set_content(Some(session_view.borrow().widget()));
        crate::ui::window::register_session_widget(session_view.borrow().widget(), Rc::clone(&session_view));

        let win_weak_trans = window.downgrade();
        let sess_weak_trans = Rc::downgrade(&session_view);
        let update_trans: Rc<dyn Fn()> = Rc::new(move || {
            if let (Some(w), Some(s)) = (win_weak_trans.upgrade(), sess_weak_trans.upgrade()) {
                if s.borrow().has_transparent_pane() {
                    w.add_css_class("transparent-window");
                } else {
                    w.remove_css_class("transparent-window");
                }
            }
        });
        crate::ui::window::register_transparency_updater(&window, update_trans);

        window.connect_close_request(|win| {
            win.set_visible(false);
            glib::Propagation::Stop
        });

        let session_weak = Rc::downgrade(&session_view);
        let window_weak = window.downgrade();
        session_view.borrow().set_action_handler(move |action| {
            let s_weak = session_weak.clone();
            let w_weak = window_weak.clone();
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
                                win.set_visible(false);
                            }
                            session.borrow_mut().reset();
                        }
                    }
                    SessionAction::Focus(id) => {
                        session.borrow_mut().set_active_pane(id);
                    }
                }
            });
        });

        Self {
            window,
            session_view,
        }
    }

    pub fn window(&self) -> &adw::ApplicationWindow {
        &self.window
    }

    pub fn session_view(&self) -> Rc<RefCell<SessionView>> {
        Rc::clone(&self.session_view)
    }

    pub fn is_visible(&self) -> bool {
        self.window.is_visible()
    }

    pub fn present(&self) {
        self.window.present();
        if let Some(active_id) = self.session_view.borrow().active_pane_id() {
            self.session_view.borrow().set_active_pane(active_id);
        }
    }

    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    pub fn toggle_visibility(&self) {
        if self.is_visible() && self.window.is_active() {
            self.hide();
        } else {
            self.present();
        }
    }

    pub fn update_transparency(&self) {
        if self.session_view.borrow().has_transparent_pane() {
            self.window.add_css_class("transparent-window");
        } else {
            self.window.remove_css_class("transparent-window");
        }
    }
}
