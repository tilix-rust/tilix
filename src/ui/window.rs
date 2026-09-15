use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use adw::prelude::*;
use gio::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;

use crate::model::{Direction, SessionLayoutTemplate, SplitOrientation};
use crate::ui::session_view::{SessionAction, SessionView};

type SessionMap = Rc<RefCell<HashMap<adw::TabPage, Rc<RefCell<SessionView>>>>>;

pub struct TilixWindow {
    window: adw::ApplicationWindow,
    tab_view: adw::TabView,
    sessions: SessionMap,
    next_session_id: Rc<RefCell<u64>>,
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
    app.set_accels_for_action("win.new-tab", &["<Primary><Shift>t"]);
    app.set_accels_for_action("win.close-pane", &["<Primary><Shift>w"]);
    app.set_accels_for_action("win.tab-next", &["<Primary>Page_Down"]);
    app.set_accels_for_action("win.tab-prev", &["<Primary>Page_Up"]);
    app.set_accels_for_action("win.split-right", &["<Primary><Shift>r"]);
    app.set_accels_for_action("win.split-down", &["<Primary><Shift>d"]);
    app.set_accels_for_action("win.balance-layout", &["<Primary><Shift>b"]);
    app.set_accels_for_action("win.toggle-sync-input", &["<Primary><Shift>i"]);
    app.set_accels_for_action("win.focus-up", &["<Alt>Up"]);
    app.set_accels_for_action("win.focus-down", &["<Alt>Down"]);
    app.set_accels_for_action("win.focus-left", &["<Alt>Left"]);
    app.set_accels_for_action("win.focus-right", &["<Alt>Right"]);

    for i in 1..=9 {
        let action_name = format!("win.switch-tab-{}", i);
        let accel = format!("<Alt>{}", i);
        app.set_accels_for_action(&action_name, &[&accel]);
    }
}

impl TilixWindow {
    pub fn new(app: &adw::Application) -> Self {
        let window = adw::ApplicationWindow::new(app);
        window.set_default_size(900, 600);
        window.set_title(Some("Tilix"));

        let header_bar = adw::HeaderBar::new();
        let title_widget = adw::WindowTitle::new("Tilix", "");
        header_bar.set_title_widget(Some(&title_widget));

        let new_tab_btn = gtk::Button::from_icon_name("tab-new-symbolic");
        new_tab_btn.set_tooltip_text(Some("New Tab (Ctrl+Shift+T)"));
        new_tab_btn.set_action_name(Some("win.new-tab"));
        new_tab_btn.add_css_class("flat");
        header_bar.pack_start(&new_tab_btn);

        let split_h_btn = gtk::Button::from_icon_name("object-flip-horizontal-symbolic");
        split_h_btn.set_tooltip_text(Some("Split Right (Ctrl+Shift+R)"));
        split_h_btn.set_action_name(Some("win.split-right"));
        split_h_btn.add_css_class("flat");
        header_bar.pack_start(&split_h_btn);

        let split_v_btn = gtk::Button::from_icon_name("object-flip-vertical-symbolic");
        split_v_btn.set_tooltip_text(Some("Split Down (Ctrl+Shift+D)"));
        split_v_btn.set_action_name(Some("win.split-down"));
        split_v_btn.add_css_class("flat");
        header_bar.pack_start(&split_v_btn);

        let sync_btn = gtk::ToggleButton::new();
        sync_btn.set_icon_name("network-transmit-receive-symbolic");
        sync_btn.set_tooltip_text(Some("Toggle Synchronized Input (Ctrl+Shift+I)"));
        sync_btn.add_css_class("flat");
        sync_btn.set_action_name(Some("win.toggle-sync-input"));
        header_bar.pack_start(&sync_btn);

        let close_btn = gtk::Button::from_icon_name("window-close-symbolic");
        close_btn.set_tooltip_text(Some("Close Pane or Tab (Ctrl+Shift+W)"));
        close_btn.set_action_name(Some("win.close-pane"));
        close_btn.add_css_class("flat");
        header_bar.pack_end(&close_btn);

        let tab_view = adw::TabView::new();
        let tab_bar = adw::TabBar::new();
        tab_bar.set_view(Some(&tab_view));
        tab_bar.set_autohide(true);

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header_bar);
        toolbar_view.add_top_bar(&tab_bar);
        toolbar_view.set_content(Some(&tab_view));
        window.set_content(Some(&toolbar_view));

        let sessions = Rc::new(RefCell::new(HashMap::new()));
        let next_session_id = Rc::new(RefCell::new(1u64));

        let tilix_win = Self {
            window,
            tab_view,
            sessions,
            next_session_id,
        };

        tilix_win.setup_tab_close_handler();
        tilix_win.setup_actions();

        // Open initial tab
        tilix_win.create_tab();

        tilix_win
    }

    fn create_tab_internal(
        tab_view: &adw::TabView,
        sessions: &SessionMap,
        next_session_id: &Rc<RefCell<u64>>,
        model: Option<crate::model::SessionModel>,
    ) -> (adw::TabPage, Rc<RefCell<SessionView>>) {
        let session_view = match model {
            Some(m) => Rc::new(RefCell::new(SessionView::with_model(m))),
            None => {
                let initial_pane_id = {
                    let mut id = next_session_id.borrow_mut();
                    let cur = *id;
                    *id += 100; // Offset pane IDs by 100 per tab to prevent any pane ID collisions
                    crate::model::PaneId(cur)
                };
                Rc::new(RefCell::new(SessionView::with_id(initial_pane_id)))
            }
        };

        let tab_page = tab_view.append(session_view.borrow().widget());

        // Set initial title and bind title changes
        let initial_title = session_view.borrow().active_title();
        tab_page.set_title(&initial_title);

        let page_weak = tab_page.downgrade();
        session_view.borrow().connect_title_changed(move |title| {
            if let Some(page) = page_weak.upgrade() {
                page.set_title(title);
            }
        });

        // Wire session view action handler
        let session_weak = Rc::downgrade(&session_view);
        let tab_page_weak = tab_page.downgrade();
        let tab_view_weak = tab_view.downgrade();
        session_view.borrow().set_action_handler(move |action| {
            let s_weak = session_weak.clone();
            let p_weak = tab_page_weak.clone();
            let tv_weak = tab_view_weak.clone();
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
                            if let (Some(tv), Some(p)) = (tv_weak.upgrade(), p_weak.upgrade()) {
                                tv.close_page(&p);
                            }
                        }
                    }
                    SessionAction::Focus(id) => {
                        session.borrow_mut().set_active_pane(id);
                    }
                }
            });
        });

        sessions
            .borrow_mut()
            .insert(tab_page.clone(), Rc::clone(&session_view));
        tab_view.set_selected_page(&tab_page);

        (tab_page, session_view)
    }

    pub fn create_tab(&self) -> (adw::TabPage, Rc<RefCell<SessionView>>) {
        Self::create_tab_internal(
            &self.tab_view,
            &self.sessions,
            &self.next_session_id,
            None,
        )
    }

    pub fn import_session_template(
        &self,
        template: &SessionLayoutTemplate,
    ) -> (adw::TabPage, Rc<RefCell<SessionView>>) {
        let (start_pane_id, start_split_id) = {
            let mut id = self.next_session_id.borrow_mut();
            let p_id = *id;
            let s_id = *id;
            *id += 100;
            (p_id, s_id)
        };
        let session_model = template.instantiate_session(start_pane_id, start_split_id);
        Self::create_tab_internal(
            &self.tab_view,
            &self.sessions,
            &self.next_session_id,
            Some(session_model),
        )
    }

    pub fn import_session_layout_from_json(
        &self,
        json: &str,
    ) -> Result<adw::TabPage, serde_json::Error> {
        let template = SessionLayoutTemplate::from_json(json)?;
        let (page, _) = self.import_session_template(&template);
        Ok(page)
    }

    pub fn export_active_session_layout(&self) -> Option<String> {
        let session_rc = self.active_session()?;
        let session = session_rc.borrow();
        let title = session.active_title();
        let model = session.model();
        let template = SessionLayoutTemplate::from_session(&title, &model);
        template.to_json().ok()
    }

    fn setup_tab_close_handler(&self) {
        let sessions_clone = Rc::clone(&self.sessions);
        let win_weak = self.window.downgrade();

        self.tab_view.connect_close_page(move |tv, page| {
            tv.close_page_finish(page, true);
            sessions_clone.borrow_mut().remove(page);

            if tv.n_pages() == 0 {
                if let Some(win) = win_weak.upgrade() {
                    win.close();
                }
            }

            glib::Propagation::Stop
        });
    }

    fn active_session(&self) -> Option<Rc<RefCell<SessionView>>> {
        let page = self.tab_view.selected_page()?;
        self.sessions.borrow().get(&page).cloned()
    }

    fn setup_actions(&self) {
        // New Tab
        {
            let action = gio::SimpleAction::new("new-tab", None);
            let tv_weak = self.tab_view.downgrade();
            let sessions_weak = Rc::downgrade(&self.sessions);
            let next_id_weak = Rc::downgrade(&self.next_session_id);

            action.connect_activate(move |_, _| {
                let Some(tv) = tv_weak.upgrade() else { return; };
                let Some(sessions) = sessions_weak.upgrade() else { return; };
                let Some(next_id) = next_id_weak.upgrade() else { return; };

                Self::create_tab_internal(&tv, &sessions, &next_id, None);
            });
            self.window.add_action(&action);
        }

        // Close Pane / Tab
        {
            let action = gio::SimpleAction::new("close-pane", None);
            let tab_view_weak = self.tab_view.downgrade();
            let sessions_weak = Rc::downgrade(&self.sessions);
            let win_weak = self.window.downgrade();

            action.connect_activate(move |_, _| {
                let Some(tv) = tab_view_weak.upgrade() else { return; };
                let Some(page) = tv.selected_page() else { return; };
                let Some(sessions) = sessions_weak.upgrade() else { return; };

                let session_opt = sessions.borrow().get(&page).cloned();
                if let Some(session) = session_opt {
                    if session.borrow().pane_count() > 1 {
                        session.borrow().close_active();
                    } else {
                        tv.close_page(&page);
                    }
                } else if let Some(win) = win_weak.upgrade() {
                    win.close();
                }
            });
            self.window.add_action(&action);
        }

        // Close Tab
        {
            let action = gio::SimpleAction::new("close-tab", None);
            let tab_view_weak = self.tab_view.downgrade();
            action.connect_activate(move |_, _| {
                let Some(tv) = tab_view_weak.upgrade() else { return; };
                if let Some(page) = tv.selected_page() {
                    tv.close_page(&page);
                }
            });
            self.window.add_action(&action);
        }

        // Export Session Layout
        {
            let action = gio::SimpleAction::new("export-session-layout", None);
            let tab_view_weak = self.tab_view.downgrade();
            let sessions_weak = Rc::downgrade(&self.sessions);

            action.connect_activate(move |_, _| {
                let Some(tv) = tab_view_weak.upgrade() else { return; };
                let Some(page) = tv.selected_page() else { return; };
                let Some(sessions) = sessions_weak.upgrade() else { return; };
                let session_opt = sessions.borrow().get(&page).cloned();
                if let Some(session) = session_opt {
                    let s = session.borrow();
                    let title = s.active_title();
                    let model = s.model();
                    let template = SessionLayoutTemplate::from_session(&title, &model);
                    if let Ok(json) = template.to_json() {
                        let mut path = glib::user_config_dir();
                        path.push("tilix");
                        path.push("templates");
                        let _ = std::fs::create_dir_all(&path);
                        let file_name = format!("{}.json", title.replace(['/', '\\', ' '], "_"));
                        let _ = std::fs::write(path.join(&file_name), &json);
                        let _ = std::fs::write(path.join("latest.json"), &json);
                        if let Some(display) = gtk::gdk::Display::default() {
                            display.clipboard().set_text(&json);
                        }
                    }
                }
            });
            self.window.add_action(&action);
        }

        // Import Session Layout
        {
            let action = gio::SimpleAction::new("import-session-layout", None);
            let tv_weak = self.tab_view.downgrade();
            let sessions_weak = Rc::downgrade(&self.sessions);
            let next_id_weak = Rc::downgrade(&self.next_session_id);

            action.connect_activate(move |_, _| {
                let Some(tv) = tv_weak.upgrade() else { return; };
                let Some(sessions) = sessions_weak.upgrade() else { return; };
                let Some(next_id) = next_id_weak.upgrade() else { return; };

                let mut template_opt = None;
                let mut path = glib::user_config_dir();
                path.push("tilix");
                path.push("templates");
                path.push("latest.json");
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(tpl) = SessionLayoutTemplate::from_json(&content) {
                        template_opt = Some(tpl);
                    }
                }

                let template = template_opt.unwrap_or_else(|| {
                    let mut tree = crate::model::LayoutTree::new(crate::model::PaneId(1));
                    let _ = tree.split(
                        crate::model::PaneId(1),
                        SplitOrientation::Horizontal,
                        crate::model::PaneId(2),
                    );
                    SessionLayoutTemplate::new("Default Layout", None, tree)
                });

                let (start_pane_id, start_split_id) = {
                    let mut id = next_id.borrow_mut();
                    let p_id = *id;
                    let s_id = *id;
                    *id += 100;
                    (p_id, s_id)
                };
                let session_model = template.instantiate_session(start_pane_id, start_split_id);
                Self::create_tab_internal(&tv, &sessions, &next_id, Some(session_model));
            });
            self.window.add_action(&action);
        }

        // Split Right
        {
            let action = gio::SimpleAction::new("split-right", None);
            let tab_view_weak = self.tab_view.downgrade();
            let sessions_weak = Rc::downgrade(&self.sessions);
            action.connect_activate(move |_, _| {
                let Some(tv) = tab_view_weak.upgrade() else { return; };
                let Some(page) = tv.selected_page() else { return; };
                let Some(sessions) = sessions_weak.upgrade() else { return; };
                let session_opt = sessions.borrow().get(&page).cloned();
                if let Some(session) = session_opt {
                    session.borrow_mut().split_active(SplitOrientation::Horizontal);
                }
            });
            self.window.add_action(&action);
        }

        // Split Down
        {
            let action = gio::SimpleAction::new("split-down", None);
            let tab_view_weak = self.tab_view.downgrade();
            let sessions_weak = Rc::downgrade(&self.sessions);
            action.connect_activate(move |_, _| {
                let Some(tv) = tab_view_weak.upgrade() else { return; };
                let Some(page) = tv.selected_page() else { return; };
                let Some(sessions) = sessions_weak.upgrade() else { return; };
                let session_opt = sessions.borrow().get(&page).cloned();
                if let Some(session) = session_opt {
                    session.borrow_mut().split_active(SplitOrientation::Vertical);
                }
            });
            self.window.add_action(&action);
        }

        // Balance Layout
        {
            let action = gio::SimpleAction::new("balance-layout", None);
            let tab_view_weak = self.tab_view.downgrade();
            let sessions_weak = Rc::downgrade(&self.sessions);
            action.connect_activate(move |_, _| {
                let Some(tv) = tab_view_weak.upgrade() else { return; };
                let Some(page) = tv.selected_page() else { return; };
                let Some(sessions) = sessions_weak.upgrade() else { return; };
                let session_opt = sessions.borrow().get(&page).cloned();
                if let Some(session) = session_opt {
                    session.borrow_mut().balance_layout();
                }
            });
            self.window.add_action(&action);
        }

        // Toggle Sync Input
        {
            let action = gio::SimpleAction::new_stateful(
                "toggle-sync-input",
                None,
                &false.to_variant(),
            );
            let tab_view_weak = self.tab_view.downgrade();
            let sessions_weak = Rc::downgrade(&self.sessions);

            action.connect_activate(move |act, _| {
                let current = act.state().and_then(|s| s.get::<bool>()).unwrap_or(false);
                let new_state = !current;
                act.set_state(&new_state.to_variant());

                let Some(tv) = tab_view_weak.upgrade() else { return; };
                let Some(page) = tv.selected_page() else { return; };
                let Some(sessions) = sessions_weak.upgrade() else { return; };
                let session_opt = sessions.borrow().get(&page).cloned();
                if let Some(session) = session_opt {
                    session.borrow_mut().set_sync_input_enabled(new_state);
                }
            });
            self.window.add_action(&action);

            // Sync action state with active tab when tab selection changes
            let action_weak = action.downgrade();
            let sessions_for_sync = Rc::clone(&self.sessions);
            self.tab_view.connect_selected_page_notify(move |tv| {
                let Some(page) = tv.selected_page() else { return; };
                let Some(act) = action_weak.upgrade() else { return; };
                let session_opt = sessions_for_sync.borrow().get(&page).cloned();
                if let Some(session) = session_opt {
                    let is_sync = session.borrow().sync_input_enabled();
                    act.set_state(&is_sync.to_variant());
                }
            });
        }

        // Tab Next
        {
            let action = gio::SimpleAction::new("tab-next", None);
            let tab_view_weak = self.tab_view.downgrade();
            action.connect_activate(move |_, _| {
                let Some(tv) = tab_view_weak.upgrade() else { return; };
                let n = tv.n_pages();
                if n > 1 {
                    if let Some(page) = tv.selected_page() {
                        let pos = tv.page_position(&page);
                        let next_pos = (pos + 1) % n;
                        let next_page = tv.nth_page(next_pos);
                        tv.set_selected_page(&next_page);
                    }
                }
            });
            self.window.add_action(&action);
        }

        // Tab Prev
        {
            let action = gio::SimpleAction::new("tab-prev", None);
            let tab_view_weak = self.tab_view.downgrade();
            action.connect_activate(move |_, _| {
                let Some(tv) = tab_view_weak.upgrade() else { return; };
                let n = tv.n_pages();
                if n > 1 {
                    if let Some(page) = tv.selected_page() {
                        let pos = tv.page_position(&page);
                        let prev_pos = if pos == 0 { n - 1 } else { pos - 1 };
                        let prev_page = tv.nth_page(prev_pos);
                        tv.set_selected_page(&prev_page);
                    }
                }
            });
            self.window.add_action(&action);
        }

        // Switch Tab 1..9
        for i in 1..=9 {
            let action_name = format!("switch-tab-{}", i);
            let action = gio::SimpleAction::new(&action_name, None);
            let tab_view_weak = self.tab_view.downgrade();
            let idx = i - 1;
            action.connect_activate(move |_, _| {
                let Some(tv) = tab_view_weak.upgrade() else { return; };
                if tv.n_pages() > idx {
                    let page = tv.nth_page(idx);
                    tv.set_selected_page(&page);
                }
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
            let tab_view_weak = self.tab_view.downgrade();
            let sessions_weak = Rc::downgrade(&self.sessions);
            action.connect_activate(move |_, _| {
                let Some(tv) = tab_view_weak.upgrade() else { return; };
                let Some(page) = tv.selected_page() else { return; };
                let Some(sessions) = sessions_weak.upgrade() else { return; };
                let session_opt = sessions.borrow().get(&page).cloned();
                if let Some(session) = session_opt {
                    session.borrow_mut().focus_adjacent(dir);
                }
            });
            self.window.add_action(&action);
        }
    }

    pub fn window(&self) -> &adw::ApplicationWindow {
        &self.window
    }

    pub fn tab_view(&self) -> &adw::TabView {
        &self.tab_view
    }

    pub fn session_view(&self) -> Option<Rc<RefCell<SessionView>>> {
        self.active_session()
    }

    pub fn present(&self) {
        self.window.present();
    }
}
