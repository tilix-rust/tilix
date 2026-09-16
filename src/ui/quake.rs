use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;

use crate::ui::session_view::SessionView;

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
        window.set_content(Some(session_view.borrow().widget()));

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
}
