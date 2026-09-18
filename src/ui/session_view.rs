use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;

use crate::model::{
    AppConfig, ColorScheme, Direction, DockPosition, LayoutNode, LayoutTree, PaneId,
    PaneTitleStyle, Profile, SessionModel, SplitOrientation,
};
use crate::ui::terminal_pane::TerminalPane;

#[derive(Debug, Clone)]
pub enum SessionAction {
    Split(PaneId, SplitOrientation),
    Close(PaneId),
    Focus(PaneId),
}

pub type ActionHandler = Box<dyn Fn(SessionAction)>;
pub type TitleChangedHandler = Box<dyn Fn(&str)>;
pub type SwapHandler = Box<dyn Fn(PaneId, PaneId)>;
pub type DockHandler = Box<dyn Fn(PaneId, PaneId, DockPosition)>;

#[derive(Clone)]
pub struct SessionView {
    container: gtk::Box,
    panes: Rc<RefCell<HashMap<PaneId, TerminalPane>>>,
    model: Rc<RefCell<SessionModel>>,
    action_handler: Rc<RefCell<Option<ActionHandler>>>,
    title_changed_callback: Rc<RefCell<Option<TitleChangedHandler>>>,
    dock_handler: Rc<RefCell<Option<DockHandler>>>,
    use_wide_handle: Rc<RefCell<bool>>,
    pane_title_style: Rc<RefCell<PaneTitleStyle>>,
    pane_title_show_when_single: Rc<RefCell<bool>>,
}

impl SessionView {
    pub fn new() -> Self {
        Self::with_id_and_dir(PaneId(1), None)
    }

    pub fn with_id(initial_pane_id: PaneId) -> Self {
        Self::with_id_and_dir(initial_pane_id, None)
    }

    pub fn with_id_and_dir(initial_pane_id: PaneId, dir: Option<&Path>) -> Self {
        let model = SessionModel::new(initial_pane_id);
        Self::with_model_and_dir(model, dir)
    }

    pub fn with_model(model: SessionModel) -> Self {
        Self::with_model_and_dir(model, None)
    }

    pub fn with_existing_pane(pane: TerminalPane) -> Self {
        let model = SessionModel::new(pane.pane_id());
        Self::with_model_and_existing_pane(model, pane)
    }

    pub fn with_model_and_existing_pane(model: SessionModel, pane: TerminalPane) -> Self {
        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        container.set_vexpand(true);
        container.set_hexpand(true);

        let model = Rc::new(RefCell::new(model));
        let panes = Rc::new(RefCell::new(HashMap::new()));
        let action_handler = Rc::new(RefCell::new(None));
        let title_changed_callback = Rc::new(RefCell::new(None));
        let dock_handler = Rc::new(RefCell::new(None));
        let cfg = AppConfig::load();
        let use_wide_handle = Rc::new(RefCell::new(cfg.use_wide_handle));
        let pane_title_style = Rc::new(RefCell::new(cfg.pane_title_style));
        let pane_title_show_when_single = Rc::new(RefCell::new(cfg.pane_title_show_when_single));

        let session = Self {
            container,
            panes,
            model,
            action_handler,
            title_changed_callback,
            dock_handler,
            use_wide_handle,
            pane_title_style,
            pane_title_show_when_single,
        };

        session.setup_dock_handler();

        let pane_id = pane.pane_id();
        pane.clear_callbacks();
        Self::wire_pane_callbacks(
            &pane,
            &session.panes,
            &session.model,
            &session.action_handler,
            &session.title_changed_callback,
            &session.dock_handler,
        );
        session.panes.borrow_mut().insert(pane_id, pane);
        session.rebuild_projection();
        session.set_active_pane(pane_id);

        session
    }

    pub fn with_model_and_dir(model: SessionModel, dir: Option<&Path>) -> Self {
        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        container.set_vexpand(true);
        container.set_hexpand(true);

        let model = Rc::new(RefCell::new(model));
        let panes = Rc::new(RefCell::new(HashMap::new()));
        let action_handler = Rc::new(RefCell::new(None));
        let title_changed_callback = Rc::new(RefCell::new(None));
        let dock_handler = Rc::new(RefCell::new(None));
        let cfg = AppConfig::load();
        let use_wide_handle = Rc::new(RefCell::new(cfg.use_wide_handle));
        let pane_title_style = Rc::new(RefCell::new(cfg.pane_title_style));
        let pane_title_show_when_single = Rc::new(RefCell::new(cfg.pane_title_show_when_single));

        let session = Self {
            container,
            panes,
            model,
            action_handler,
            title_changed_callback,
            dock_handler,
            use_wide_handle,
            pane_title_style,
            pane_title_show_when_single,
        };

        session.setup_dock_handler();

        let pane_ids = session.model.borrow().layout.panes();
        for id in pane_ids {
            let pane = Self::create_pane(
                id,
                dir,
                &session.panes,
                &session.model,
                &session.action_handler,
                &session.title_changed_callback,
                &session.dock_handler,
            );
            session.panes.borrow_mut().insert(id, pane);
        }

        session.rebuild_projection();
        session
    }

    fn setup_dock_handler(&self) {
        let panes_w = Rc::downgrade(&self.panes);
        let model_w = Rc::downgrade(&self.model);
        let action_w = Rc::downgrade(&self.action_handler);
        let title_w = Rc::downgrade(&self.title_changed_callback);
        let dock_w = Rc::downgrade(&self.dock_handler);
        let wide_w = Rc::downgrade(&self.use_wide_handle);
        let style_w = Rc::downgrade(&self.pane_title_style);
        let show_w = Rc::downgrade(&self.pane_title_show_when_single);
        let container = self.container.clone();

        *self.dock_handler.borrow_mut() = Some(Box::new(move |src, dest, pos| {
            let Some(panes) = panes_w.upgrade() else { return; };
            let Some(model) = model_w.upgrade() else { return; };
            let Some(action_handler) = action_w.upgrade() else { return; };
            let Some(title_changed_callback) = title_w.upgrade() else { return; };
            let Some(dock_handler) = dock_w.upgrade() else { return; };
            let Some(use_wide_handle) = wide_w.upgrade() else { return; };
            let Some(pane_title_style) = style_w.upgrade() else { return; };
            let Some(pane_title_show_when_single) = show_w.upgrade() else { return; };

            let sv = SessionView {
                container: container.clone(),
                panes,
                model,
                action_handler,
                title_changed_callback,
                dock_handler,
                use_wide_handle,
                pane_title_style,
                pane_title_show_when_single,
            };
            sv.dock_pane(src, dest, pos);
        }));
    }

    pub fn set_action_handler<F: Fn(SessionAction) + 'static>(&self, f: F) {
        *self.action_handler.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_title_changed<F: Fn(&str) + 'static>(&self, f: F) {
        *self.title_changed_callback.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_dock_handler<F: Fn(PaneId, PaneId, DockPosition) + 'static>(&self, f: F) {
        *self.dock_handler.borrow_mut() = Some(Box::new(f));
    }

    fn wire_pane_callbacks(
        pane: &TerminalPane,
        panes: &Rc<RefCell<HashMap<PaneId, TerminalPane>>>,
        model: &Rc<RefCell<SessionModel>>,
        action_handler: &Rc<RefCell<Option<ActionHandler>>>,
        title_changed_callback: &Rc<RefCell<Option<TitleChangedHandler>>>,
        dock_handler: &Rc<RefCell<Option<DockHandler>>>,
    ) {
        pane.setup_drag_source();
        let dock_cb = Rc::clone(dock_handler);
        pane.setup_drop_target(move |src, dest, pos| {
            if let Ok(cb_ref) = dock_cb.try_borrow() {
                if let Some(ref cb) = *cb_ref {
                    cb(src, dest, pos);
                }
            }
        });

        let handler = Rc::clone(action_handler);
        pane.connect_close(move |p_id| {
            if let Ok(h) = handler.try_borrow() {
                if let Some(cb) = h.as_ref() {
                    cb(SessionAction::Close(p_id));
                }
            }
        });

        let handler_split = Rc::clone(action_handler);
        pane.connect_split(move |p_id, orient| {
            if let Ok(h) = handler_split.try_borrow() {
                if let Some(cb) = h.as_ref() {
                    cb(SessionAction::Split(p_id, orient));
                }
            }
        });

        let panes_focus = panes.clone();
        let model_focus = Rc::clone(model);
        let title_cb_focus = Rc::clone(title_changed_callback);
        let handler_focus = Rc::clone(action_handler);
        pane.connect_focus(move |p_id| {
            if let Ok(panes) = panes_focus.try_borrow() {
                if panes.contains_key(&p_id) {
                    if let Ok(mut m) = model_focus.try_borrow_mut() {
                        m.set_active_pane(p_id);
                    }
                    for (id, p) in panes.iter() {
                        p.set_active(*id == p_id);
                    }
                    if let Some(p) = panes.get(&p_id) {
                        let title = p.title();
                        if let Ok(cb_ref) = title_cb_focus.try_borrow() {
                            if let Some(ref cb) = *cb_ref {
                                cb(&title);
                            }
                        }
                    }
                }
            }
            if let Ok(h) = handler_focus.try_borrow() {
                if let Some(cb) = h.as_ref() {
                    cb(SessionAction::Focus(p_id));
                }
            }
        });

        // Sync toggle callback: update model
        let model_sync = Rc::clone(model);
        pane.connect_sync_toggled(move |p_id, enabled| {
            if let Ok(mut m) = model_sync.try_borrow_mut() {
                m.set_pane_sync_enabled(p_id, enabled);
            }
        });

        // Input broadcasting: commit callback
        let panes_weak = Rc::downgrade(panes);
        let panes_commit = panes_weak.clone();
        let model_commit = Rc::clone(model);
        pane.connect_commit(move |sender_id, text| {
            let Some(panes_rc) = panes_commit.upgrade() else { return; };
            let Ok(model) = model_commit.try_borrow() else { return; };
            if !model.sync_input_enabled {
                return;
            }
            let Ok(panes) = panes_rc.try_borrow() else { return; };
            let sender_sync = panes
                .get(&sender_id)
                .map(|p| p.is_sync_enabled())
                .unwrap_or(false);
            if !sender_sync {
                return;
            }
            for (target_id, target_pane) in panes.iter() {
                if *target_id != sender_id && target_pane.is_sync_enabled() {
                    target_pane.feed_child(text.as_bytes());
                }
            }
        });

        // Title update callback
        let model_title = Rc::clone(model);
        let title_cb = Rc::clone(title_changed_callback);
        pane.connect_title_changed(move |p_id, title| {
            if let Ok(model) = model_title.try_borrow() {
                if model.active_pane == Some(p_id) {
                    if let Ok(cb_ref) = title_cb.try_borrow() {
                        if let Some(ref cb) = *cb_ref {
                            cb(title);
                        }
                    }
                }
            }
        });

        // Bell notification
        let model_bell = Rc::clone(model);
        let title_label_bell = pane.title_label().downgrade();
        pane.connect_bell(move |p_id| {
            let is_active = model_bell
                .try_borrow()
                .map(|m| m.active_pane == Some(p_id))
                .unwrap_or(false);
            let cfg = crate::model::AppConfig::load();
            if cfg.notifications_enabled && cfg.bell_notifications && !is_active {
                let title = title_label_bell
                    .upgrade()
                    .map(|l| l.text().to_string())
                    .unwrap_or_else(|| "Terminal".to_string());
                if let Some(app) = gio::Application::default().and_then(|a| a.downcast::<adw::Application>().ok()) {
                    crate::ui::notifications::NotificationService::notify_bell(&app, &title);
                }
            }
        });

        // Process exit notification
        let model_exit = Rc::clone(model);
        let title_label_exit = pane.title_label().downgrade();
        pane.connect_child_exited(move |p_id, status| {
            let is_active = model_exit
                .try_borrow()
                .map(|m| m.active_pane == Some(p_id))
                .unwrap_or(false);
            let cfg = crate::model::AppConfig::load();
            if cfg.notifications_enabled && cfg.process_exit_notifications && !is_active {
                let title = title_label_exit
                    .upgrade()
                    .map(|l| l.text().to_string())
                    .unwrap_or_else(|| "Terminal".to_string());
                if let Some(app) = gio::Application::default().and_then(|a| a.downcast::<adw::Application>().ok()) {
                    crate::ui::notifications::NotificationService::notify_process_exit(&app, &title, status);
                }
            }
        });
    }

    fn create_pane(
        id: PaneId,
        initial_directory: Option<&Path>,
        panes: &Rc<RefCell<HashMap<PaneId, TerminalPane>>>,
        model: &Rc<RefCell<SessionModel>>,
        action_handler: &Rc<RefCell<Option<ActionHandler>>>,
        title_changed_callback: &Rc<RefCell<Option<TitleChangedHandler>>>,
        dock_handler: &Rc<RefCell<Option<DockHandler>>>,
    ) -> TerminalPane {
        let pane = TerminalPane::new(id, initial_directory);
        let cfg = crate::model::AppConfig::load();
        pane.apply_profile(cfg.get_default_profile());

        Self::wire_pane_callbacks(
            &pane,
            panes,
            model,
            action_handler,
            title_changed_callback,
            dock_handler,
        );

        pane
    }

    pub fn widget(&self) -> &gtk::Widget {
        self.container.upcast_ref()
    }

    pub fn container(&self) -> &gtk::Box {
        &self.container
    }

    pub fn panes(&self) -> Rc<RefCell<HashMap<PaneId, TerminalPane>>> {
        Rc::clone(&self.panes)
    }

    pub fn model(&self) -> std::cell::Ref<'_, SessionModel> {
        self.model.borrow()
    }

    pub fn model_mut(&self) -> std::cell::RefMut<'_, SessionModel> {
        self.model.borrow_mut()
    }

    pub fn layout(&self) -> LayoutTree {
        self.model.borrow().layout.clone()
    }

    pub fn is_empty(&self) -> bool {
        self.panes.borrow().is_empty()
    }

    fn detach_widget(w: &gtk::Widget) {
        if let Some(parent) = w.parent() {
            if let Ok(paned) = parent.downcast::<gtk::Paned>() {
                if paned.start_child().as_ref() == Some(w) {
                    paned.set_start_child(None::<&gtk::Widget>);
                } else if paned.end_child().as_ref() == Some(w) {
                    paned.set_end_child(None::<&gtk::Widget>);
                } else {
                    w.unparent();
                }
            } else {
                w.unparent();
            }
        }
    }

    pub fn reset(&self) {
        let initial_pane_id = PaneId(1);
        *self.model.borrow_mut() = SessionModel::new(initial_pane_id);
        let old_panes: Vec<TerminalPane> = self.panes.borrow_mut().drain().map(|(_, p)| p).collect();
        for p in &old_panes {
            p.close();
            let w = p.widget();
            Self::detach_widget(w);
        }
        drop(old_panes);
        let pane = Self::create_pane(
            initial_pane_id,
            None,
            &self.panes,
            &self.model,
            &self.action_handler,
            &self.title_changed_callback,
            &self.dock_handler,
        );
        self.panes.borrow_mut().insert(initial_pane_id, pane);
        self.rebuild_projection();
    }

    pub fn close(&self) {
        let old_panes: Vec<TerminalPane> = self.panes.borrow_mut().drain().map(|(_, p)| p).collect();
        for p in &old_panes {
            p.close();
            let w = p.widget();
            Self::detach_widget(w);
        }
    }

    pub fn pane_count(&self) -> usize {
        self.panes.borrow().len()
    }

    pub fn active_pane_id(&self) -> Option<PaneId> {
        self.model.borrow().active_pane
    }

    pub fn sync_input_enabled(&self) -> bool {
        self.model.borrow().sync_input_enabled
    }

    pub fn set_sync_input_enabled(&self, enabled: bool) {
        self.model.borrow_mut().sync_input_enabled = enabled;
    }

    pub fn toggle_sync_input(&self) -> bool {
        self.model.borrow_mut().toggle_sync_input()
    }

    pub fn active_title(&self) -> String {
        let active_id = self.model.borrow().active_pane;
        if let Some(id) = active_id {
            if let Some(pane) = self.panes.borrow().get(&id) {
                return pane.title();
            }
        }
        "Terminal".to_string()
    }

    pub fn set_active_pane(&self, id: PaneId) {
        let panes = self.panes.borrow();
        if panes.contains_key(&id) {
            self.model.borrow_mut().set_active_pane(id);
            for (pane_id, pane) in panes.iter() {
                pane.set_active(*pane_id == id);
            }
            if let Some(pane) = panes.get(&id) {
                pane.grab_focus();
                let title = pane.title();
                if let Some(ref cb) = *self.title_changed_callback.borrow() {
                    cb(&title);
                }
            }
        }
    }

    pub fn active_current_directory(&self) -> Option<PathBuf> {
        let active_id = self.model.borrow().active_pane?;
        let panes = self.panes.borrow();
        let pane = panes.get(&active_id)?;
        pane.current_directory()
    }

    pub fn swap_panes(&self, a: PaneId, b: PaneId) {
        self.capture_layout_ratios();
        let swapped = self.model.borrow_mut().layout.swap_panes(a, b).is_ok();
        if swapped {
            self.rebuild_projection();
        }
    }

    pub fn remove_pane_for_transfer(&self, id: PaneId) -> Option<TerminalPane> {
        if !self.panes.borrow().contains_key(&id) {
            return None;
        }
        self.capture_layout_ratios();
        let next_focus = self.model.borrow_mut().remove_pane(id).ok().flatten();
        let pane = self.panes.borrow_mut().remove(&id)?;
        Self::detach_widget(pane.widget());
        if let Some(act_id) = next_focus {
            self.set_active_pane(act_id);
        }
        Some(pane)
    }

    pub fn adopt_pane(&self, pane: TerminalPane, target_id: PaneId, position: DockPosition) {
        let pane_id = pane.pane_id();
        self.capture_layout_ratios();
        pane.clear_callbacks();
        Self::wire_pane_callbacks(
            &pane,
            &self.panes,
            &self.model,
            &self.action_handler,
            &self.title_changed_callback,
            &self.dock_handler,
        );
        self.panes.borrow_mut().insert(pane_id, pane);
        let _ = self.model.borrow_mut().adopt_pane(pane_id, target_id, position);
        self.rebuild_projection();
        self.set_active_pane(pane_id);
    }

    pub fn dock_pane(&self, src_id: PaneId, dest_id: PaneId, position: DockPosition) {
        self.capture_layout_ratios();
        if self.panes.borrow().contains_key(&src_id) {
            let res = self.model.borrow_mut().dock_pane(src_id, dest_id, position);
            if res.is_ok() {
                self.rebuild_projection();
                self.set_active_pane(src_id);
            }
        } else {
            let Some(drag) = crate::ui::dnd::get_active_pane_drag() else { return; };
            if drag.pane_id != src_id {
                return;
            }
            let Some(source_widget) = drag.source_session_widget.upgrade() else { return; };
            let Some(source_session_rc) = crate::ui::window::session_for_widget(&source_widget) else { return; };

            let Some(pane) = source_session_rc.borrow().remove_pane_for_transfer(src_id) else { return; };
            source_session_rc.borrow().rebuild_projection();
            if source_session_rc.borrow().is_empty() {
                crate::ui::window::close_session_tab(&source_widget);
            }

            self.adopt_pane(pane, dest_id, position);
        }
    }

    pub fn grab_focus(&self) {
        if let Some(id) = self.model.borrow().active_pane {
            if let Some(pane) = self.panes.borrow().get(&id) {
                pane.grab_focus();
            }
        }
    }

    pub fn split_active(&self, orientation: SplitOrientation) {
        let active_pane = self.model.borrow().active_pane;
        let target_pane = match active_pane {
            Some(id) if self.panes.borrow().contains_key(&id) => id,
            _ => {
                if let Some(&first_id) = self.panes.borrow().keys().next() {
                    self.model.borrow_mut().set_active_pane(first_id);
                    first_id
                } else {
                    return;
                }
            }
        };

        let dir = self.panes.borrow().get(&target_pane).and_then(|p| p.current_directory());
        self.split_pane_with_dir(target_pane, orientation, dir.as_deref());
    }

    pub fn split_pane(&self, target: PaneId, orientation: SplitOrientation) {
        let dir = self.panes.borrow().get(&target).and_then(|p| p.current_directory());
        self.split_pane_with_dir(target, orientation, dir.as_deref());
    }

    pub fn split_pane_with_dir(
        &self,
        target: PaneId,
        orientation: SplitOrientation,
        dir: Option<&Path>,
    ) {
        self.capture_layout_ratios();
        let res = self.model.borrow_mut().split_pane(target, orientation);
        if let Ok(new_id) = res {
            let new_pane = Self::create_pane(
                new_id,
                dir,
                &self.panes,
                &self.model,
                &self.action_handler,
                &self.title_changed_callback,
                &self.dock_handler,
            );
            self.panes.borrow_mut().insert(new_id, new_pane);
            self.rebuild_projection();
            self.set_active_pane(new_id);
        }
    }

    pub fn close_active(&self) {
        let active_id = self.model.borrow().active_pane;
        if let Some(id) = active_id {
            self.close_pane(id);
        }
    }

    pub fn close_pane(&self, id: PaneId) {
        let has_pane = self.panes.borrow().contains_key(&id);
        if !has_pane {
            return;
        }

        self.capture_layout_ratios();
        if self.model.borrow_mut().close_pane(id).is_ok() {
            let pane = self.panes.borrow_mut().remove(&id);
            if let Some(pane) = pane {
                pane.close();
                let w = pane.widget();
                Self::detach_widget(w);
            }
            self.rebuild_projection();
            let next_active = self.model.borrow().active_pane;
            if let Some(act_id) = next_active {
                self.set_active_pane(act_id);
            }
        }
    }

    pub fn focus_adjacent(&self, direction: Direction) {
        let adjacent = self.model.borrow_mut().focus_adjacent(direction);
        if let Some(target) = adjacent {
            self.set_active_pane(target);
        }
    }

    pub fn balance_layout(&self) {
        self.model.borrow_mut().layout.balance();
        self.rebuild_projection();
    }

    pub fn apply_profile(&self, profile: &Profile) {
        for pane in self.panes.borrow().values() {
            pane.apply_profile(profile);
        }
    }

    pub fn apply_profile_to_pane(&self, pane_id: PaneId, profile: &Profile) {
        if let Some(pane) = self.panes.borrow().get(&pane_id) {
            pane.apply_profile(profile);
        }
    }

    pub fn get_pane_profile(&self, pane_id: PaneId) -> Option<Profile> {
        self.panes.borrow().get(&pane_id).map(|p| p.current_profile())
    }

    pub fn apply_color_scheme(&self, scheme: &ColorScheme) {
        for pane in self.panes.borrow().values() {
            pane.apply_color_scheme(scheme);
        }
    }

    pub fn set_wide_handle(&self, wide: bool) {
        *self.use_wide_handle.borrow_mut() = wide;
        let mut child = self.container.first_child();
        while let Some(c) = child {
            Self::set_paneds_wide_handle(&c, wide);
            child = c.next_sibling();
        }
    }

    pub fn use_wide_handle(&self) -> bool {
        *self.use_wide_handle.borrow()
    }

    pub fn update_pane_headers_visibility(&self) {
        let count = self.pane_count();
        let style = *self.pane_title_style.borrow();
        let show_single = *self.pane_title_show_when_single.borrow();
        let visible = match style {
            PaneTitleStyle::None => false,
            PaneTitleStyle::Normal => {
                if count <= 1 {
                    show_single
                } else {
                    true
                }
            }
        };
        for pane in self.panes.borrow().values() {
            pane.set_header_visible(visible);
        }
    }

    pub fn set_pane_title_settings(&self, style: PaneTitleStyle, show_when_single: bool) {
        *self.pane_title_style.borrow_mut() = style;
        *self.pane_title_show_when_single.borrow_mut() = show_when_single;
        self.update_pane_headers_visibility();
    }

    pub fn pane_title_style(&self) -> PaneTitleStyle {
        *self.pane_title_style.borrow()
    }

    pub fn pane_title_show_when_single(&self) -> bool {
        *self.pane_title_show_when_single.borrow()
    }

    fn set_paneds_wide_handle(widget: &gtk::Widget, wide: bool) {
        if let Ok(paned) = widget.clone().downcast::<gtk::Paned>() {
            paned.set_wide_handle(wide);
            if let Some(start) = paned.start_child() {
                Self::set_paneds_wide_handle(&start, wide);
            }
            if let Some(end) = paned.end_child() {
                Self::set_paneds_wide_handle(&end, wide);
            }
        }
    }

    pub fn capture_layout_ratios(&self) {
        if let Some(root_widget) = self.container.first_child() {
            if let Ok(mut m) = self.model.try_borrow_mut() {
                if let Some(root_node) = m.layout.root_mut() {
                    Self::capture_ratios_recursive(root_node, &root_widget);
                }
            }
        }
    }

    fn capture_ratios_recursive(node: &mut LayoutNode, widget: &gtk::Widget) {
        if let LayoutNode::Split {
            ratio,
            first,
            second,
            ..
        } = node
        {
            if let Some(paned) = widget.downcast_ref::<gtk::Paned>() {
                let len = match paned.orientation() {
                    gtk::Orientation::Horizontal => paned.width(),
                    gtk::Orientation::Vertical => paned.height(),
                    _ => paned.width(),
                };
                let pos = paned.position();
                if len > 0 && pos > 0 {
                    let r = (pos as f64 / len as f64).clamp(0.05, 0.95);
                    *ratio = r;
                }
                if let Some(c1) = paned.start_child() {
                    Self::capture_ratios_recursive(first, &c1);
                }
                if let Some(c2) = paned.end_child() {
                    Self::capture_ratios_recursive(second, &c2);
                }
            }
        }
    }

    fn build_node(
        node: &LayoutNode,
        panes: &HashMap<PaneId, TerminalPane>,
        model: &Rc<RefCell<SessionModel>>,
        container: &gtk::Box,
        wide_handle: bool,
        avail_w: i32,
        avail_h: i32,
    ) -> gtk::Widget {
        match node {
            LayoutNode::Leaf(id) => {
                if let Some(pane) = panes.get(id) {
                    let w = pane.widget();
                    Self::detach_widget(w);
                    w.clone()
                } else {
                    gtk::Box::new(gtk::Orientation::Vertical, 0).upcast()
                }
            }
            LayoutNode::Split {
                id,
                orientation,
                ratio,
                first,
                second,
            } => {
                let gtk_orientation = match orientation {
                    SplitOrientation::Horizontal => gtk::Orientation::Horizontal,
                    SplitOrientation::Vertical => gtk::Orientation::Vertical,
                };
                let r = *ratio;
                let (child1_w, child1_h, child2_w, child2_h, initial_pos) = match orientation {
                    SplitOrientation::Horizontal => {
                        let pos = if avail_w > 0 {
                            (avail_w as f64 * r).round() as i32
                        } else {
                            0
                        };
                        (pos, avail_h, (avail_w - pos).max(0), avail_h, pos)
                    }
                    SplitOrientation::Vertical => {
                        let pos = if avail_h > 0 {
                            (avail_h as f64 * r).round() as i32
                        } else {
                            0
                        };
                        (avail_w, pos, avail_w, (avail_h - pos).max(0), pos)
                    }
                };

                let first_widget = Self::build_node(first, panes, model, container, wide_handle, child1_w, child1_h);
                let second_widget = Self::build_node(second, panes, model, container, wide_handle, child2_w, child2_h);

                let paned = gtk::Paned::new(gtk_orientation);
                paned.set_wide_handle(wide_handle);
                paned.set_shrink_start_child(false);
                paned.set_shrink_end_child(false);
                paned.set_resize_start_child(true);
                paned.set_resize_end_child(true);

                paned.set_start_child(Some(&first_widget));
                paned.set_end_child(Some(&second_widget));

                if initial_pos > 0 {
                    paned.set_position(initial_pos);
                }

                // Reorder separator to be the last child of GtkPaned.
                // In GTK4, gtk_widget_pick evaluates children in reverse order (last to first).
                // With handle as the last child, handle_contains (which insets the 1px handle
                // by 6px on all sides) is tested FIRST before start_child and end_child.
                // This gives an exact, symmetric 6px hover/click hit area on BOTH sides of the 1px line!
                let mut handle = None;
                let mut c = paned.first_child();
                while let Some(child) = c {
                    if child.css_name() == "separator" {
                        handle = Some(child);
                        break;
                    }
                    c = child.next_sibling();
                }
                if let (Some(ref h), Some(ref end)) = (&handle, paned.end_child()) {
                    h.insert_after(&paned, Some(end));
                }

                paned.connect_map(move |p| {
                    let p_weak = p.downgrade();
                    glib::idle_add_local_once(move || {
                        if let Some(p) = p_weak.upgrade() {
                            let len = match p.orientation() {
                                gtk::Orientation::Horizontal => p.width(),
                                gtk::Orientation::Vertical => p.height(),
                                _ => p.width(),
                            };
                            if len > 0 {
                                p.set_position((len as f64 * r).round() as i32);
                            }
                        }
                    });
                });

                // Also run once via idle immediately in case widget is already mapped
                let paned_init_weak = paned.downgrade();
                glib::idle_add_local_once(move || {
                    if let Some(p) = paned_init_weak.upgrade() {
                        let len = match p.orientation() {
                            gtk::Orientation::Horizontal => p.width(),
                            gtk::Orientation::Vertical => p.height(),
                            _ => p.width(),
                        };
                        if len > 0 {
                            p.set_position((len as f64 * r).round() as i32);
                        }
                    }
                });

                // Locate GtkPaned's built-in drag gesture
                let controllers = paned.observe_controllers();
                let mut drag_opt = None;
                for i in 0..controllers.n_items() {
                    if let Some(item) = controllers.item(i) {
                        if let Some(drag) = item.downcast_ref::<gtk::GestureDrag>() {
                            drag_opt = Some(drag.clone());
                        }
                    }
                }

                let is_dragging = Rc::new(std::cell::Cell::new(false));
                let just_equalized = Rc::new(std::cell::Cell::new(false));
                let split_id = *id;
                let split_orientation = *orientation;
                let model_clone = Rc::clone(model);

                let container_weak = container.downgrade();
                let paned_weak = paned.downgrade();
                let model_for_equalize = Rc::clone(model);
                let just_eq_action = Rc::clone(&just_equalized);
                let equalize_action = Rc::new(move || {
                    just_eq_action.set(true);
                    if let Ok(mut m) = model_for_equalize.try_borrow_mut() {
                        m.layout.equalize_direction(split_orientation);
                    }
                    if let Some(container) = container_weak.upgrade() {
                        let w = container.width();
                        let h = container.height();
                        let model_b = model_for_equalize.borrow();
                        if let Some(root_node) = model_b.layout.root() {
                            if let Some(root_widget) = container.first_child() {
                                Self::apply_ratios_recursive(root_node, &root_widget, w, h);
                                let root_clone = root_node.clone();
                                let rw_weak = root_widget.downgrade();
                                let just_eq_idle = Rc::clone(&just_eq_action);
                                glib::idle_add_local_once(move || {
                                    if let Some(rw) = rw_weak.upgrade() {
                                        Self::apply_ratios_recursive(&root_clone, &rw, w, h);
                                    }
                                    just_eq_idle.set(false);
                                });
                            }
                        }
                    }
                });

                let last_click_time = Rc::new(std::cell::Cell::new(None::<std::time::Instant>));

                // 1. Connect to GtkPaned's native drag gesture (which GTK activates only on the separator)
                if let Some(ref drag) = drag_opt {
                    let is_dragging_begin = Rc::clone(&is_dragging);
                    let is_dragging_end = Rc::clone(&is_dragging);
                    let last_click = Rc::clone(&last_click_time);
                    let eq = Rc::clone(&equalize_action);

                    drag.connect_drag_begin(move |_gesture, _start_x, _start_y| {
                        is_dragging_begin.set(true);
                        let now = std::time::Instant::now();
                        let is_double = if let Some(prev) = last_click.get() {
                            now.duration_since(prev) < std::time::Duration::from_millis(450)
                        } else {
                            false
                        };
                        last_click.set(Some(now));
                        if is_double {
                            last_click.set(None);
                            eq();
                        }
                    });

                    let model_for_drag = Rc::clone(model);
                    let paned_for_drag = paned.downgrade();
                    drag.connect_drag_update(move |_gesture, _offset_x, _offset_y| {
                        if let Some(p) = paned_for_drag.upgrade() {
                            let len = match p.orientation() {
                                gtk::Orientation::Horizontal => p.width(),
                                gtk::Orientation::Vertical => p.height(),
                                _ => p.width(),
                            };
                            if len > 0 && p.position() > 0 {
                                let ratio = (p.position() as f64 / len as f64).clamp(0.05, 0.95);
                                if let Ok(mut m) = model_for_drag.try_borrow_mut() {
                                    m.layout.set_split_ratio(split_id, ratio);
                                }
                            }
                        }
                    });

                    let model_for_end = Rc::clone(model);
                    let paned_for_end = paned.downgrade();
                    drag.connect_drag_end(move |_gesture, _offset_x, _offset_y| {
                        is_dragging_end.set(false);
                        if let Some(p) = paned_for_end.upgrade() {
                            let len = match p.orientation() {
                                gtk::Orientation::Horizontal => p.width(),
                                gtk::Orientation::Vertical => p.height(),
                                _ => p.width(),
                            };
                            if len > 0 && p.position() > 0 {
                                let ratio = (p.position() as f64 / len as f64).clamp(0.05, 0.95);
                                if let Ok(mut m) = model_for_end.try_borrow_mut() {
                                    m.layout.set_split_ratio(split_id, ratio);
                                }
                            }
                        }
                    });
                }

                let just_eq_notify = Rc::clone(&just_equalized);
                let is_dragging_notify = Rc::clone(&is_dragging);
                paned.connect_notify_local(Some("position"), move |p, _| {
                    if just_eq_notify.get() {
                        return;
                    }
                    if is_dragging_notify.get() {
                        let len = match p.orientation() {
                            gtk::Orientation::Horizontal => p.width(),
                            gtk::Orientation::Vertical => p.height(),
                            _ => p.width(),
                        };
                        if len > 0 && p.position() > 0 {
                            let ratio = (p.position() as f64 / len as f64).clamp(0.05, 0.95);
                            if let Ok(mut m) = model_clone.try_borrow_mut() {
                                m.layout.set_split_ratio(split_id, ratio);
                            }
                        }
                    }
                });

                // 2. Attach GestureClick on paned as direct backup
                let click = gtk::GestureClick::new();
                click.set_button(gtk::gdk::BUTTON_PRIMARY);
                click.set_propagation_phase(gtk::PropagationPhase::Capture);
                paned.add_controller(click.clone());
                {
                    let last_click = Rc::clone(&last_click_time);
                    let eq = Rc::clone(&equalize_action);
                    let paned_weak = paned_weak.clone();
                    click.connect_pressed(move |gesture, n_press, x, y| {
                        let Some(p) = paned_weak.upgrade() else { return; };
                        let pos = p.position() as f64;
                        let is_on_handle = (match p.orientation() {
                            gtk::Orientation::Horizontal => (x - pos).abs() <= 10.0,
                            gtk::Orientation::Vertical => (y - pos).abs() <= 10.0,
                            _ => false,
                        }) || p.pick(x, y, gtk::PickFlags::DEFAULT).map(|w| w.css_name() == "separator").unwrap_or(false);

                        if !is_on_handle {
                            return;
                        }

                        let now = std::time::Instant::now();
                        let is_double_time = if let Some(prev) = last_click.get() {
                            now.duration_since(prev) < std::time::Duration::from_millis(450)
                        } else {
                            false
                        };
                        last_click.set(Some(now));

                        if n_press == 2 || is_double_time {
                            last_click.set(None);
                            gesture.set_state(gtk::EventSequenceState::Claimed);
                            eq();
                        }
                    });
                }

                paned.upcast()
            }
        }
    }

    fn apply_ratios_recursive(node: &LayoutNode, widget: &gtk::Widget, avail_w: i32, avail_h: i32) {
        if let LayoutNode::Split {
            orientation,
            ratio,
            first,
            second,
            ..
        } = node
        {
            if let Some(paned) = widget.downcast_ref::<gtk::Paned>() {
                let (len, is_horiz) = match orientation {
                    SplitOrientation::Horizontal => (
                        if paned.width() > 0 { paned.width() } else { avail_w },
                        true,
                    ),
                    SplitOrientation::Vertical => (
                        if paned.height() > 0 { paned.height() } else { avail_h },
                        false,
                    ),
                };
                let pos = if len > 0 {
                    let p = (len as f64 * *ratio).round() as i32;
                    paned.set_position(p);
                    p
                } else {
                    paned.position()
                };

                let (c1_w, c1_h, c2_w, c2_h) = if is_horiz {
                    (pos, avail_h, (avail_w - pos).max(0), avail_h)
                } else {
                    (avail_w, pos, avail_w, (avail_h - pos).max(0))
                };

                if let Some(c1) = paned.start_child() {
                    Self::apply_ratios_recursive(first, &c1, c1_w, c1_h);
                }
                if let Some(c2) = paned.end_child() {
                    Self::apply_ratios_recursive(second, &c2, c2_w, c2_h);
                }
            }
        }
    }

    pub fn equalize_direction(&self, orientation: SplitOrientation) {
        if let Ok(mut m) = self.model.try_borrow_mut() {
            m.layout.equalize_direction(orientation);
        }
        self.apply_layout_ratios();
        let model_clone = Rc::clone(&self.model);
        let container_weak = self.container.downgrade();
        glib::idle_add_local_once(move || {
            if let Some(container) = container_weak.upgrade() {
                let w = container.width();
                let h = container.height();
                let model_b = model_clone.borrow();
                if let Some(root_node) = model_b.layout.root() {
                    if let Some(root_widget) = container.first_child() {
                        Self::apply_ratios_recursive(root_node, &root_widget, w, h);
                    }
                }
            }
        });
    }

    pub fn apply_layout_ratios(&self) {
        let w = self.container.width();
        let h = self.container.height();
        let model_b = self.model.borrow();
        if let Some(root_node) = model_b.layout.root() {
            if let Some(root_widget) = self.container.first_child() {
                Self::apply_ratios_recursive(root_node, &root_widget, w, h);
            }
        }
    }

    fn rebuild_projection_with(
        container: &gtk::Box,
        panes: &Rc<RefCell<HashMap<PaneId, TerminalPane>>>,
        model: &Rc<RefCell<SessionModel>>,
        wide_handle: bool,
    ) {
        if let Some(root_widget) = container.first_child() {
            if let Ok(mut m) = model.try_borrow_mut() {
                if let Some(root_node) = m.layout.root_mut() {
                    Self::capture_ratios_recursive(root_node, &root_widget);
                }
            }
        }

        let panes_b = panes.borrow();
        for pane in panes_b.values() {
            let w = pane.widget();
            Self::detach_widget(w);
        }

        while let Some(child) = container.first_child() {
            Self::detach_widget(&child);
        }

        let model_b = model.borrow();
        let Some(root_node) = model_b.layout.root() else {
            return;
        };

        let avail_w = container.width();
        let avail_h = container.height();
        let root_widget = Self::build_node(root_node, &panes_b, model, container, wide_handle, avail_w, avail_h);
        root_widget.set_vexpand(true);
        root_widget.set_hexpand(true);
        container.append(&root_widget);

        let active_id = model_b.active_pane;
        drop(model_b);
        drop(panes_b);

        let panes_ref = panes.borrow();
        for (pane_id, pane) in panes_ref.iter() {
            pane.set_active(Some(*pane_id) == active_id);
        }
        if let Some(id) = active_id {
            if let Some(pane) = panes_ref.get(&id) {
                pane.grab_focus();
            }
        }
    }

    pub fn rebuild_projection(&self) {
        Self::rebuild_projection_with(
            &self.container,
            &self.panes,
            &self.model,
            *self.use_wide_handle.borrow(),
        );
        self.update_pane_headers_visibility();
    }
}

impl Default for SessionView {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SplitId;

    #[test]
    fn test_session_view_lifecycle_and_focus() {
        crate::ui::window::run_gtk_test(|| {

        // Sub-test 1: Basic lifecycle and reset
        {
            let session = SessionView::new();
            assert_eq!(session.pane_count(), 1);
            let active_id = session.active_pane_id().unwrap();

            session.split_active(SplitOrientation::Horizontal);
            assert_eq!(session.pane_count(), 2);

            session.close_pane(active_id);
            assert_eq!(session.pane_count(), 1);

            session.reset();
            assert_eq!(session.pane_count(), 1);

            session.close();
            assert!(session.is_empty());
        }

        // Sub-test 2: Closing another pane does NOT cause current pane to lose focus
        {
            let session = SessionView::new();
            assert_eq!(session.pane_count(), 1);
            let p1 = session.active_pane_id().unwrap();

            session.split_active(SplitOrientation::Horizontal);
            let p2 = session.active_pane_id().unwrap();
            assert_ne!(p1, p2);

            session.split_active(SplitOrientation::Vertical);
            let p3 = session.active_pane_id().unwrap();
            assert_ne!(p2, p3);

            // Switch active back to p1
            session.set_active_pane(p1);
            assert_eq!(session.active_pane_id(), Some(p1));

            // Close pane 2 (another pane, not the active one)
            session.close_pane(p2);
            assert_eq!(session.pane_count(), 2);
            // Current active pane MUST NOT lose focus / must remain active!
            assert_eq!(session.active_pane_id(), Some(p1));

            // Close pane 3 (another pane, not the active one)
            session.close_pane(p3);
            assert_eq!(session.pane_count(), 1);
            // Current active pane MUST NOT lose focus / must remain active!
            assert_eq!(session.active_pane_id(), Some(p1));

            session.close();
        }

        // Sub-test 3: Closing active pane automatically moves focus to previous pane
        {
            let session = SessionView::new();
            let p1 = session.active_pane_id().unwrap();

            session.split_active(SplitOrientation::Horizontal);
            let p2 = session.active_pane_id().unwrap();

            session.split_active(SplitOrientation::Vertical);
            let p3 = session.active_pane_id().unwrap();

            // Currently active pane is p3
            assert_eq!(session.active_pane_id(), Some(p3));

            // Close active pane p3: focus must automatically move to previous pane p2
            session.close_pane(p3);
            assert_eq!(session.pane_count(), 2);
            assert_eq!(session.active_pane_id(), Some(p2));

            // Close active pane p2: focus must automatically move to previous pane p1
            session.close_pane(p2);
            assert_eq!(session.pane_count(), 1);
            assert_eq!(session.active_pane_id(), Some(p1));

            session.close();
        }

        // Sub-test 4: Closing active pane with MRU history switch
        {
            let session = SessionView::new();
            let p1 = session.active_pane_id().unwrap();

            session.split_active(SplitOrientation::Horizontal);
            let p2 = session.active_pane_id().unwrap();

            session.split_active(SplitOrientation::Vertical);
            let p3 = session.active_pane_id().unwrap();

            // Switch to p1: MRU order becomes [p2, p3, p1]
            session.set_active_pane(p1);
            assert_eq!(session.active_pane_id(), Some(p1));

            // Closing current pane p1 must automatically move focus to previous pane p3
            session.close_pane(p1);
            assert_eq!(session.pane_count(), 2);
            assert_eq!(session.active_pane_id(), Some(p3));

            // Closing current pane p3 must automatically move focus to previous pane p2
            session.close_pane(p3);
            assert_eq!(session.pane_count(), 1);
            assert_eq!(session.active_pane_id(), Some(p2));

            session.close();
        }

        // Sub-test 5: Terminal pane header buttons are not focusable to prevent stealing focus on click
        {
            let session = SessionView::new();
            let active_id = session.active_pane_id().unwrap();
            let panes = session.panes();
            let panes_ref = panes.borrow();
            let pane = panes_ref.get(&active_id).unwrap();

            assert!(!pane.close_button().is_focusable());
            assert!(!pane.split_h_button().is_focusable());
            assert!(!pane.split_v_button().is_focusable());
            assert!(!pane.sync_button().is_focusable());

            drop(panes_ref);
            session.close();
        }

        // Sub-test 6: Dynamic splitter wide handle toggle
        {
            let session = SessionView::new();
            session.split_active(SplitOrientation::Horizontal);
            assert_eq!(session.pane_count(), 2);

            session.set_wide_handle(true);
            assert!(session.use_wide_handle());
            let child = session.container.first_child().expect("container child exists");
            let paned = child.downcast::<gtk::Paned>().expect("child should be gtk::Paned");
            assert!(paned.is_wide_handle());

            session.set_wide_handle(false);
            assert!(!session.use_wide_handle());
            assert!(!paned.is_wide_handle());

            session.close();
        }
        });
    }

    #[test]
    fn test_session_view_set_wide_handle() {
        crate::ui::window::run_gtk_test(|| {
            let session = SessionView::new();
            session.split_active(SplitOrientation::Horizontal);
            assert_eq!(session.pane_count(), 2);

            session.set_wide_handle(true);
            assert!(session.use_wide_handle());
            let child = session.container.first_child().expect("container child exists");
            let paned = child.downcast::<gtk::Paned>().expect("child should be gtk::Paned");
            assert!(paned.is_wide_handle());

            session.set_wide_handle(false);
            assert!(!session.use_wide_handle());
            assert!(!paned.is_wide_handle());

            session.close();
        });
    }

    #[test]
    fn test_session_view_pane_title_none_hides_all_headers() {
        crate::ui::window::run_gtk_test(|| {
            let session = SessionView::new();
            session.split_active(SplitOrientation::Horizontal);
            assert_eq!(session.pane_count(), 2);

            // By default, PaneTitleStyle::Normal and headers are visible
            for pane in session.panes().borrow().values() {
                assert!(pane.is_header_visible());
            }

            // Set PaneTitleStyle::None -> hides all headers
            session.set_pane_title_settings(PaneTitleStyle::None, true);
            assert_eq!(session.pane_title_style(), PaneTitleStyle::None);
            for pane in session.panes().borrow().values() {
                assert!(!pane.is_header_visible());
            }

            // Restore PaneTitleStyle::Normal -> headers visible again (multi-pane)
            session.set_pane_title_settings(PaneTitleStyle::Normal, true);
            assert_eq!(session.pane_title_style(), PaneTitleStyle::Normal);
            for pane in session.panes().borrow().values() {
                assert!(pane.is_header_visible());
            }

            session.close();
        });
    }

    #[test]
    fn test_session_view_show_when_single_false_dynamic_split() {
        crate::ui::window::run_gtk_test(|| {
            let session = SessionView::new();
            assert_eq!(session.pane_count(), 1);

            // Configure show_when_single = false
            session.set_pane_title_settings(PaneTitleStyle::Normal, false);
            assert!(!session.pane_title_show_when_single());

            // 1 pane: header should be hidden
            for pane in session.panes().borrow().values() {
                assert!(!pane.is_header_visible());
            }

            // Split active: now 2 panes, both should have visible headers
            session.split_active(SplitOrientation::Horizontal);
            assert_eq!(session.pane_count(), 2);
            for pane in session.panes().borrow().values() {
                assert!(pane.is_header_visible());
            }

            // Close active pane: back to 1 pane, header should be hidden again
            session.close_active();
            assert_eq!(session.pane_count(), 1);
            for pane in session.panes().borrow().values() {
                assert!(!pane.is_header_visible());
            }

            // Enable show_when_single = true: single pane header becomes visible
            session.set_pane_title_settings(PaneTitleStyle::Normal, true);
            assert!(session.pane_title_show_when_single());
            for pane in session.panes().borrow().values() {
                assert!(pane.is_header_visible());
            }

            session.close();
        });
    }

    #[test]
    fn test_session_view_dock_intra_session() {
        crate::ui::window::run_gtk_test(|| {
            let session = SessionView::new();
            let p1 = session.active_pane_id().unwrap();
            session.split_active(SplitOrientation::Horizontal);
            assert_eq!(session.pane_count(), 2);
            let p2 = session.active_pane_id().unwrap();

            session.dock_pane(p1, p2, DockPosition::Bottom);
            assert_eq!(session.pane_count(), 2);
            assert_eq!(session.active_pane_id(), Some(p1));
            assert_eq!(session.model().layout.panes(), vec![p2, p1]);

            session.close();
        });
    }

    #[test]
    fn test_session_view_transfer_pane() {
        crate::ui::window::run_gtk_test(|| {
            let session_a = SessionView::with_id(PaneId(1));
            session_a.split_active(SplitOrientation::Horizontal);
            assert_eq!(session_a.pane_count(), 2);
            let p2 = session_a.active_pane_id().unwrap();

            let session_b = SessionView::with_id(PaneId(10));
            assert_eq!(session_b.pane_count(), 1);

            let pane_to_transfer = session_a.remove_pane_for_transfer(p2).unwrap();
            session_a.rebuild_projection();
            assert_eq!(session_a.pane_count(), 1);

            session_b.adopt_pane(pane_to_transfer, PaneId(10), DockPosition::Right);
            assert_eq!(session_b.pane_count(), 2);
            assert_eq!(session_b.active_pane_id(), Some(p2));
            assert_eq!(session_b.model().layout.panes(), vec![PaneId(10), p2]);

            session_a.close();
            session_b.close();
        });
    }

    #[test]
    fn test_session_view_equalize_direction_horizontal() {
        crate::ui::window::run_gtk_test(|| {
            let session = SessionView::new();
            session.split_active(SplitOrientation::Horizontal);
            session.split_active(SplitOrientation::Horizontal);
            assert_eq!(session.pane_count(), 3);

            // Mess up ratios in layout
            session.model_mut().layout.set_split_ratio(SplitId(1), 0.8);
            session.model_mut().layout.set_split_ratio(SplitId(2), 0.2);

            // Call equalize_direction on Horizontal
            session.equalize_direction(SplitOrientation::Horizontal);

            // Model should have equalized ratios (1/3 and 1/2)
            if let Some(LayoutNode::Split { ratio: r1, second, .. }) = session.model().layout.root() {
                assert!((r1 - (1.0 / 3.0)).abs() < 1e-6);
                if let LayoutNode::Split { ratio: r2, .. } = &**second {
                    assert!((r2 - 0.5).abs() < 1e-6);
                } else {
                    panic!("Expected second node to be split");
                }
            } else {
                panic!("Expected root to be split");
            }

            session.close();
        });
    }

    #[test]
    fn test_session_view_equalize_direction_vertical() {
        crate::ui::window::run_gtk_test(|| {
            let session = SessionView::new();
            session.split_active(SplitOrientation::Vertical);
            assert_eq!(session.pane_count(), 2);

            // Mess up ratio
            session.model_mut().layout.set_split_ratio(SplitId(1), 0.85);

            // Call equalize_direction on Vertical
            session.equalize_direction(SplitOrientation::Vertical);

            if let Some(LayoutNode::Split { ratio, .. }) = session.model().layout.root() {
                assert!((ratio - 0.5).abs() < 1e-6);
            } else {
                panic!("Expected root to be split");
            }

            session.close();
        });
    }

    #[test]
    fn test_session_view_paned_separator_gesture_attached() {
        crate::ui::window::run_gtk_test(|| {
            crate::ui::window::setup_css();
            let session = SessionView::new();
            session.split_active(SplitOrientation::Horizontal);
            assert_eq!(session.pane_count(), 2);

            let root_widget = session.container.first_child().expect("root widget must exist");
            let paned = root_widget.downcast_ref::<gtk::Paned>().expect("root widget must be GtkPaned");

            let mut handle = None;
            let mut c = paned.first_child();
            while let Some(child) = c {
                if child.css_name() == "separator" {
                    handle = Some(child);
                    break;
                }
                c = child.next_sibling();
            }
            assert!(handle.is_some(), "Separator handle must exist in GtkPaned");

            let controllers = paned.observe_controllers();
            let mut drag_controller = None;
            for i in 0..controllers.n_items() {
                if let Some(item) = controllers.item(i) {
                    if let Some(drag) = item.downcast_ref::<gtk::GestureDrag>() {
                        drag_controller = Some(drag.clone());
                    }
                }
            }
            let drag = drag_controller.expect("GtkGestureDrag must exist");

            let window = gtk::Window::new();
            window.set_default_size(400, 300);
            window.set_child(Some(session.widget()));
            window.present();

            let ctx = glib::MainContext::default();
            for _ in 0..10 {
                ctx.iteration(false);
            }

            // Simulate drag_begin, drag_update, drag_end
            use glib::prelude::*;
            drag.emit_by_name::<()>("drag-begin", &[&200.0f64, &100.0f64]);
            paned.set_position(280);
            drag.emit_by_name::<()>("drag-update", &[&80.0f64, &0.0f64]);
            drag.emit_by_name::<()>("drag-end", &[&80.0f64, &0.0f64]);

            for _ in 0..10 {
                ctx.iteration(false);
            }

            if let LayoutNode::Split { ratio, .. } = session.model.borrow().layout.root().unwrap() {
                assert!(*ratio != 0.5, "Model ratio must be updated upon separator drag");
            }

            session.close();
        });
    }

    #[test]
    fn test_paned_hit_test_symmetry() {
        crate::ui::window::run_gtk_test(|| {
            crate::ui::window::setup_css();
            let window = gtk::Window::new();
            window.set_default_size(400, 300);
            let paned = gtk::Paned::new(gtk::Orientation::Horizontal);
            let b1 = gtk::Button::new();
            let b2 = gtk::Button::new();
            paned.set_start_child(Some(&b1));
            paned.set_end_child(Some(&b2));
            paned.set_position(200);
            window.set_child(Some(&paned));
            window.present();

            let ctx = glib::MainContext::default();
            for _ in 0..10 {
                ctx.iteration(false);
            }

            let mut handle = None;
            let mut c = paned.first_child();
            while let Some(child) = c {
                if child.css_name() == "separator" {
                    handle = Some(child);
                    break;
                }
                c = child.next_sibling();
            }

            if let (Some(ref h), Some(ref end)) = (&handle, paned.end_child()) {
                h.insert_after(&paned, Some(end));
            }

            // Hit test: line is drawn at x = 200 (width = 1px).
            // Left of line: [194..200] (6px) picked as separator.
            // Right of line: [201..207] (6px) picked as separator.
            // Outside: < 194 or > 207 picked as button.
            for x in [194.0, 196.0, 198.0, 200.0, 201.0, 204.0, 207.0] {
                let picked = paned.pick(x, 150.0, gtk::PickFlags::DEFAULT);
                assert_eq!(
                    picked.map(|w| w.css_name().to_string()),
                    Some("separator".to_string()),
                    "x={} should be picked as separator",
                    x
                );
            }
            // Outside the 6px margin
            assert_eq!(
                paned.pick(193.0, 150.0, gtk::PickFlags::DEFAULT).map(|w| w.css_name().to_string()),
                Some("button".to_string())
            );
            assert_eq!(
                paned.pick(208.0, 150.0, gtk::PickFlags::DEFAULT).map(|w| w.css_name().to_string()),
                Some("button".to_string())
            );
        });
    }

    #[test]
    fn test_double_click_separator_equalizes_panes() {
        crate::ui::window::run_gtk_test(|| {
            crate::ui::window::setup_css();
            let mut model = SessionModel::new(PaneId(1));
            model.split_pane(PaneId(1), SplitOrientation::Horizontal).unwrap();
            let root = model.layout.root().unwrap().clone();
            if let LayoutNode::Split { id, .. } = root {
                model.layout.set_split_ratio(id, 0.2);
            }
            let session = SessionView::with_model_and_dir(model, None);
            let window = gtk::Window::new();
            window.set_default_size(400, 300);
            window.set_child(Some(session.widget()));
            window.present();

            let ctx = glib::MainContext::default();
            for _ in 0..10 {
                ctx.iteration(false);
            }

            if let LayoutNode::Split { ratio, .. } = session.model.borrow().layout.root().unwrap() {
                assert!((*ratio - 0.2).abs() < 0.01, "Initial ratio should be 0.2, got {}", ratio);
            }

            let paned = session.container.first_child().unwrap().downcast::<gtk::Paned>().unwrap();
            let controllers = paned.observe_controllers();
            let mut click_opt = None;
            for i in 0..controllers.n_items() {
                if let Some(item) = controllers.item(i) {
                    if let Some(click) = item.downcast_ref::<gtk::GestureClick>() {
                        click_opt = Some(click.clone());
                    }
                }
            }
            let click = click_opt.expect("GestureClick must be attached to paned");

            // Clicking away from separator (x=10.0 in left pane) with n_press=2 should NOT equalize
            use glib::prelude::*;
            click.emit_by_name::<()>("pressed", &[&2i32, &10.0f64, &100.0f64]);
            click.emit_by_name::<()>("released", &[&2i32, &10.0f64, &100.0f64]);
            for _ in 0..5 {
                ctx.iteration(false);
            }
            if let LayoutNode::Split { ratio, .. } = session.model.borrow().layout.root().unwrap() {
                assert!((*ratio - 0.2).abs() < 0.01, "Ratio should remain 0.2 when clicking in pane");
            }

            // Double clicking on separator (pos) should equalize to 0.5
            let pos = paned.position() as f64;
            click.emit_by_name::<()>("pressed", &[&2i32, &pos, &100.0f64]);
            for _ in 0..10 {
                ctx.iteration(false);
            }
            if let LayoutNode::Split { ratio, .. } = session.model.borrow().layout.root().unwrap() {
                assert!((*ratio - 0.5).abs() < 0.01, "Ratio must be equalized to 0.5, got {}", ratio);
            }

            // Now reset back to 0.2 and test time-based double click with n_press = 1
            let root = session.model.borrow().layout.root().unwrap().clone();
            if let LayoutNode::Split { id, .. } = root {
                session.model.borrow_mut().layout.set_split_ratio(id, 0.2);
            }
            session.apply_layout_ratios();
            for _ in 0..5 {
                ctx.iteration(false);
            }

            // Two single-clicks (n_press=1) within 450ms triggers time-based double click
            click.emit_by_name::<()>("pressed", &[&1i32, &pos, &100.0f64]);
            click.emit_by_name::<()>("pressed", &[&1i32, &pos, &100.0f64]);
            for _ in 0..10 {
                ctx.iteration(false);
            }
            if let LayoutNode::Split { ratio, .. } = session.model.borrow().layout.root().unwrap() {
                assert!((*ratio - 0.5).abs() < 0.01, "Time-based double click must equalize to 0.5, got {}", ratio);
            }

            session.close();
        });
    }

    #[test]
    fn test_split_preserves_existing_custom_ratios() {
        crate::ui::window::run_gtk_test(|| {
            crate::ui::window::setup_css();
            let mut model = SessionModel::new(PaneId(1));
            model.split_pane(PaneId(1), SplitOrientation::Horizontal).unwrap();
            let root = model.layout.root().unwrap().clone();
            if let LayoutNode::Split { id, .. } = root {
                model.layout.set_split_ratio(id, 0.7);
            }
            let session = SessionView::with_model_and_dir(model, None);
            let window = gtk::Window::new();
            window.set_default_size(400, 300);
            window.set_child(Some(session.widget()));
            window.present();

            let ctx = glib::MainContext::default();
            for _ in 0..10 {
                ctx.iteration(false);
            }

            let paned = session.container.first_child().unwrap().downcast::<gtk::Paned>().unwrap();
            assert_eq!(paned.position(), 280);

            // Split Pane 2 vertically
            session.split_pane(PaneId(2), SplitOrientation::Vertical);
            window.present();

            for _ in 0..10 {
                ctx.iteration(false);
            }

            let root = session.model.borrow().layout.root().unwrap().clone();
            if let LayoutNode::Split { ratio, .. } = root {
                assert!((ratio - 0.7).abs() < 0.01, "First split ratio should remain 0.7, got {}", ratio);
            } else {
                panic!("Root should be Split");
            }

            let root_paned = session.container.first_child().unwrap().downcast::<gtk::Paned>().unwrap();
            assert_eq!(root_paned.position(), 280, "Root paned position must be 280 (70%), not reset/equalized");

            session.close();
        });
    }
}
