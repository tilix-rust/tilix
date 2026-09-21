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
    is_closed: Rc<std::cell::Cell<bool>>,
}

/// Dynamically calculates the target geometry for the Quake drop-down window.
/// Defaults to 100% of monitor width and `quake_height_percent` % of monitor height.
pub fn calculate_quake_dimensions(window: &adw::ApplicationWindow) -> (i32, i32) {
    let display = gtk::gdk::Display::default();
    let monitor = display.as_ref().and_then(|d| {
        if let Some(surface) = window.surface() {
            if let Some(mon) = d.monitor_at_surface(&surface) {
                return Some(mon);
            }
        }
        if let Some(app) = window.application() {
            if let Some(active_win) = app.active_window() {
                if let Some(surface) = active_win.surface() {
                    if let Some(mon) = d.monitor_at_surface(&surface) {
                        return Some(mon);
                    }
                }
            }
        }
        d.monitors().item(0).and_then(|o| o.downcast::<gtk::gdk::Monitor>().ok())
    });

    let (mon_w, mon_h) = monitor
        .map(|m| {
            let geom = m.geometry();
            (geom.width(), geom.height())
        })
        .unwrap_or((1920, 1080));

    let cfg = crate::model::AppConfig::load();
    let height = ((mon_h as f64) * (cfg.quake_height_percent.clamp(10, 100) as f64) / 100.0).round() as i32;
    (mon_w, height)
}

impl TilixQuakeWindow {
    pub fn new(app: &adw::Application) -> Self {
        let is_closed = Rc::new(std::cell::Cell::new(false));
        let window = adw::ApplicationWindow::new(app);
        window.set_title(Some("Tilix Quake"));
        window.add_css_class("quake-window");
        window.set_valign(gtk::Align::Start);
        let (width, height) = calculate_quake_dimensions(&window);
        window.set_default_size(width, height);

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

        let is_closed_close = is_closed.clone();
        window.connect_close_request(move |_| {
            is_closed_close.set(true);
            glib::Propagation::Proceed
        });

        let is_closed_destroy = is_closed.clone();
        window.connect_destroy(move |_| {
            is_closed_destroy.set(true);
        });

        window.connect_is_active_notify(move |win| {
            if !win.is_active() {
                let cfg = crate::model::AppConfig::load();
                if cfg.quake_hide_on_unfocus {
                    win.set_visible(false);
                }
            }
        });

        {
            let win_weak = window.downgrade();
            let action = gio::SimpleAction::new("quake-toggle", None);
            action.connect_activate(move |_, _| {
                if let Some(w) = win_weak.upgrade() {
                    w.set_visible(false);
                }
            });
            window.add_action(&action);
        }

        let session_weak = Rc::downgrade(&session_view);
        let window_weak = window.downgrade();
        let is_closed_action = is_closed.clone();
        session_view.borrow().set_action_handler(move |action| {
            let s_weak = session_weak.clone();
            let w_weak = window_weak.clone();
            let is_closed_inner = is_closed_action.clone();
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
                            is_closed_inner.set(true);
                            if let Some(win) = w_weak.upgrade() {
                                win.destroy();
                            }
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
            is_closed,
        }
    }

    pub fn window(&self) -> &adw::ApplicationWindow {
        &self.window
    }

    pub fn session_view(&self) -> Rc<RefCell<SessionView>> {
        Rc::clone(&self.session_view)
    }

    pub fn destroy(&self) {
        self.is_closed.set(true);
        self.window.destroy();
    }

    pub fn close(&self) {
        self.is_closed.set(true);
        self.window.close();
    }

    pub fn is_closed(&self) -> bool {
        self.is_closed.get()
    }

    pub fn is_visible(&self) -> bool {
        !self.is_closed.get() && self.window.is_visible()
    }

    pub fn present(&self) {
        if self.is_closed.get() {
            return;
        }
        let (width, height) = calculate_quake_dimensions(&self.window);
        self.window.set_default_size(width, height);
        self.window.set_visible(true);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quake_dimensions_calculation() {
        crate::ui::window::run_gtk_test(|| {
            let app = adw::Application::builder()
                .application_id("com.github.tilix_rust.test.quake_geom")
                .build();
            let window = adw::ApplicationWindow::new(&app);
            let (w, h) = calculate_quake_dimensions(&window);
            assert!(w > 0, "Quake width must be positive: {}", w);
            assert!(h > 0, "Quake height must be positive: {}", h);
        });
    }

    #[test]
    fn test_quake_hide_and_present() {
        crate::ui::window::run_gtk_test(|| {
            let app = adw::Application::builder()
                .application_id("com.github.tilix_rust.test.present_hide")
                .build();
            let q = TilixQuakeWindow::new(&app);
            q.present();
            assert!(q.is_visible());
            q.hide();
            assert!(!q.is_visible());
            q.present();
            assert!(q.is_visible(), "q.present() must make the window visible after hide!");
        });
    }

    #[test]
    fn test_quake_exit_and_reopen_cycle() {
        crate::ui::window::run_gtk_test(|| {
            let app = adw::Application::builder()
                .application_id("com.github.tilix_rust.test.reopen_cycle")
                .build();
            let quake_win = Rc::new(RefCell::new(None));
            let q1 = crate::app::get_or_create_quake(&app, &quake_win);
            q1.present();
            assert!(q1.is_visible());
            assert!(!q1.is_closed());

            // Simulate user exiting the shell (destroy window on exit)
            q1.destroy();
            while glib::MainContext::default().iteration(false) {}
            assert!(q1.is_closed());

            // Re-acquire and present Quake window
            let q2 = crate::app::get_or_create_quake(&app, &quake_win);
            assert!(!q2.is_closed());
            q2.present();
            assert!(q2.is_visible(), "New Quake window must be visible after previous was destroyed");
        });
    }

    #[test]
    fn test_quake_pane_exit_and_reopen_via_action() {
        crate::ui::window::run_gtk_test(|| {
            let app = adw::Application::builder()
                .application_id("com.github.tilix_rust.test.pane_exit_cycle")
                .build();
            let quake_win = Rc::new(RefCell::new(None));
            let q1 = crate::app::get_or_create_quake(&app, &quake_win);
            q1.present();
            assert!(q1.is_visible());
            assert!(!q1.is_closed());

            let active_id = q1.session_view().borrow().active_pane_id().unwrap();
            q1.session_view().borrow().close_pane(active_id);
            if q1.session_view().borrow().is_empty() {
                q1.destroy();
            }
            while glib::MainContext::default().iteration(false) {}
            assert!(q1.is_closed());

            let q2 = crate::app::get_or_create_quake(&app, &quake_win);
            assert!(!q2.is_closed());
            q2.present();
            assert!(q2.is_visible());
        });
    }
}

