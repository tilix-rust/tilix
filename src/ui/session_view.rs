use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk4 as gtk;
use libadwaita as adw;

use crate::model::{
    expand_title_tokens_scoped, AppConfig, ColorScheme, Direction, DockPosition, LayoutNode,
    LayoutTree, PaneId, PaneTitleStyle, Profile, SessionModel, SplitId, SplitOrientation, TitleEditScope,
    TokenContext,
};
use crate::ui::terminal_pane::TerminalPane;
use vte4::prelude::*;

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
    is_broadcasting: Rc<Cell<bool>>,
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
        let is_broadcasting = Rc::new(Cell::new(false));

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
            is_broadcasting,
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
            &session.is_broadcasting,
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
        let is_broadcasting = Rc::new(Cell::new(false));

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
            is_broadcasting,
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
                &session.is_broadcasting,
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
        let broadcasting_w = Rc::downgrade(&self.is_broadcasting);
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
            let Some(is_broadcasting) = broadcasting_w.upgrade() else { return; };

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
                is_broadcasting,
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
        is_broadcasting: &Rc<Cell<bool>>,
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
            let mut changed = false;
            if let Ok(panes) = panes_focus.try_borrow() {
                if panes.contains_key(&p_id) {
                    if let Ok(mut m) = model_focus.try_borrow_mut() {
                        if m.active_pane != Some(p_id) {
                            m.set_active_pane(p_id);
                            changed = true;
                        }
                    }
                    for (id, p) in panes.iter() {
                        p.set_active(*id == p_id);
                    }
                    if let Ok(m) = model_focus.try_borrow() {
                        let title = Self::compute_session_title_internal_from_refs(&m, &panes);
                        if let Ok(cb_ref) = title_cb_focus.try_borrow() {
                            if let Some(ref cb) = *cb_ref {
                                cb(&title);
                            }
                        }
                    }
                }
            }
            if changed {
                if let Ok(h) = handler_focus.try_borrow() {
                    if let Some(cb) = h.as_ref() {
                        cb(SessionAction::Focus(p_id));
                    }
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
        let broadcasting = Rc::clone(is_broadcasting);
        pane.connect_commit(move |sender_id, text| {
            if broadcasting.get() {
                return;
            }
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

            broadcasting.set(true);
            struct BroadcastGuard(Rc<Cell<bool>>);
            impl Drop for BroadcastGuard {
                fn drop(&mut self) {
                    self.0.set(false);
                }
            }
            let _guard = BroadcastGuard(Rc::clone(&broadcasting));

            for (target_id, target_pane) in panes.iter() {
                if *target_id != sender_id && target_pane.is_sync_enabled() {
                    target_pane.feed_child(text.as_bytes());
                }
            }
        });

        // Title update callback
        let model_title = Rc::clone(model);
        let panes_title = panes.clone();
        let title_cb = Rc::clone(title_changed_callback);
        pane.connect_title_changed(move |p_id, _title| {
            if let Ok(model) = model_title.try_borrow() {
                if model.active_pane == Some(p_id) {
                    if let Ok(panes) = panes_title.try_borrow() {
                        let title = Self::compute_session_title_internal_from_refs(&model, &panes);
                        if let Ok(cb_ref) = title_cb.try_borrow() {
                            if let Some(ref cb) = *cb_ref {
                                cb(&title);
                            }
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
        is_broadcasting: &Rc<Cell<bool>>,
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
            is_broadcasting,
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
            &self.is_broadcasting,
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

    pub fn active_pane(&self) -> Option<TerminalPane> {
        let id = self.active_pane_id()?;
        self.panes.borrow().get(&id).cloned()
    }

    pub fn zoom_in_active(&self) {
        if let Some(pane) = self.active_pane() {
            pane.zoom_in();
        }
    }

    pub fn zoom_out_active(&self) {
        if let Some(pane) = self.active_pane() {
            pane.zoom_out();
        }
    }

    pub fn zoom_normal_active(&self) {
        if let Some(pane) = self.active_pane() {
            pane.zoom_normal();
        }
    }

    pub fn copy_clipboard_active(&self) {
        if let Some(pane) = self.active_pane() {
            pane.copy_clipboard();
        }
    }

    pub fn copy_html_active(&self) {
        if let Some(pane) = self.active_pane() {
            pane.copy_html();
        }
    }

    pub fn paste_clipboard_active(&self) {
        if let Some(pane) = self.active_pane() {
            pane.paste_clipboard();
        }
    }

    pub fn paste_primary_active(&self) {
        if let Some(pane) = self.active_pane() {
            pane.paste_primary();
        }
    }

    pub fn select_all_active(&self) {
        if let Some(pane) = self.active_pane() {
            pane.select_all();
        }
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

    pub fn build_token_context(&self) -> TokenContext {
        let model = self.model.borrow();
        let panes = self.panes.borrow();
        let terminal_count = panes.len();

        let mut pane_ids: Vec<PaneId> = panes.keys().copied().collect();
        pane_ids.sort();

        let active_id = model.active_pane;
        let terminal_number = active_id
            .and_then(|id| pane_ids.iter().position(|&p| p == id).map(|idx| idx + 1))
            .unwrap_or(1);

        if let Some(id) = active_id {
            if let Some(pane) = panes.get(&id) {
                let p_title = pane.title();
                let cols = pane.terminal().column_count();
                let rows = pane.terminal().row_count();
                return TokenContext {
                    title: p_title.clone(),
                    icon_title: Some(p_title.clone()),
                    id: Some(pane.pane_id().0),
                    directory: pane.current_directory(),
                    hostname: None,
                    username: None,
                    columns: if cols > 0 { Some(cols as u32) } else { None },
                    rows: if rows > 0 { Some(rows as u32) } else { None },
                    process: None,
                    readonly: false,
                    silence: false,
                    input_sync: pane.is_sync_enabled(),
                    profile_name: Some(pane.current_profile().name),
                    active_terminal_title: Some(p_title),
                    terminal_count: Some(terminal_count.max(1)),
                    terminal_number: Some(terminal_number),
                    ..Default::default()
                };
            }
        }

        TokenContext::new_session("Terminal", terminal_count.max(1), terminal_number)
    }

    fn compute_session_title_internal_from_refs(
        model: &SessionModel,
        panes: &HashMap<PaneId, TerminalPane>,
    ) -> String {
        let terminal_count = panes.len();
        let mut pane_ids: Vec<PaneId> = panes.keys().copied().collect();
        pane_ids.sort();

        let active_id = model.active_pane;
        let terminal_number = active_id
            .and_then(|id| pane_ids.iter().position(|&p| p == id).map(|idx| idx + 1))
            .unwrap_or(1);

        let ctx = if let Some(id) = active_id {
            if let Some(pane) = panes.get(&id) {
                let p_title = pane.title();
                let cols = pane.terminal().column_count();
                let rows = pane.terminal().row_count();
                TokenContext {
                    title: p_title.clone(),
                    icon_title: Some(p_title.clone()),
                    id: Some(pane.pane_id().0),
                    directory: pane.current_directory(),
                    hostname: None,
                    username: None,
                    columns: if cols > 0 { Some(cols as u32) } else { None },
                    rows: if rows > 0 { Some(rows as u32) } else { None },
                    process: None,
                    readonly: false,
                    silence: false,
                    input_sync: pane.is_sync_enabled(),
                    profile_name: Some(pane.current_profile().name),
                    active_terminal_title: Some(p_title),
                    terminal_count: Some(terminal_count.max(1)),
                    terminal_number: Some(terminal_number),
                    ..Default::default()
                }
            } else {
                TokenContext::new_session("Terminal", terminal_count.max(1), terminal_number)
            }
        } else {
            TokenContext::new_session("Terminal", terminal_count.max(1), terminal_number)
        };

        let cfg = AppConfig::load();
        expand_title_tokens_scoped(
            &cfg.default_session_name,
            TitleEditScope::Session,
            &ctx,
        )
    }

    pub fn compute_session_title(&self) -> String {
        Self::compute_session_title_internal_from_refs(&self.model.borrow(), &self.panes.borrow())
    }

    pub fn active_title(&self) -> String {
        self.compute_session_title()
    }

    pub fn notify_title_changed(&self) {
        let title = self.compute_session_title();
        if let Ok(cb_ref) = self.title_changed_callback.try_borrow() {
            if let Some(ref cb) = *cb_ref {
                cb(&title);
            }
        }
    }

    pub fn set_active_pane(&self, id: PaneId) {
        let panes = self.panes.borrow();
        if panes.contains_key(&id) {
            self.model.borrow_mut().set_active_pane(id);
            for (pane_id, pane) in panes.iter() {
                pane.set_active(*pane_id == id);
            }
            if let Some(pane) = panes.get(&id) {
                if !pane.terminal().has_focus() {
                    pane.grab_focus();
                }
            }
            drop(panes);
            self.notify_title_changed();
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
            &self.is_broadcasting,
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
        let active_id = self.model.borrow().active_pane;
        let target_id = match active_id {
            Some(id) if self.panes.borrow().contains_key(&id) => Some(id),
            _ => self.panes.borrow().keys().next().copied(),
        };
        if let Some(id) = target_id {
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
                &self.is_broadcasting,
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
        if let Ok(mut m) = self.model.try_borrow_mut() {
            m.layout.balance();
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

    pub fn apply_profile(&self, profile: &Profile) {
        for pane in self.panes.borrow().values() {
            pane.apply_profile(profile);
        }
        self.notify_title_changed();
    }

    pub fn apply_profile_to_pane(&self, pane_id: PaneId, profile: &Profile) {
        if let Some(pane) = self.panes.borrow().get(&pane_id) {
            pane.apply_profile(profile);
        }
        self.notify_title_changed();
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

    fn is_point_on_paned_handle(p: &gtk::Paned, x: f64, y: f64) -> bool {
        let pos = p.position() as f64;
        let near_pos = match p.orientation() {
            gtk::Orientation::Horizontal => (x - pos).abs() <= 10.0,
            gtk::Orientation::Vertical => (y - pos).abs() <= 10.0,
            _ => false,
        };
        if let Some(w) = p.pick(x, y, gtk::PickFlags::DEFAULT) {
            if w.css_name() == "separator" {
                return w.parent().as_ref() == Some(p.upcast_ref());
            }
            near_pos
        } else {
            near_pos
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
                paned.set_widget_name(&format!("split-{}", id.0));
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

                let initial_applied = std::rc::Rc::new(std::cell::Cell::new(initial_pos > 0));
                let applied_map = std::rc::Rc::clone(&initial_applied);
                paned.connect_map(move |p| {
                    if applied_map.get() {
                        return;
                    }
                    let len = match p.orientation() {
                        gtk::Orientation::Horizontal => p.width(),
                        gtk::Orientation::Vertical => p.height(),
                        _ => p.width(),
                    };
                    if len > 0 {
                        p.set_position((len as f64 * r).round() as i32);
                        applied_map.set(true);
                    } else {
                        let p_weak = p.downgrade();
                        let applied_idle = std::rc::Rc::clone(&applied_map);
                        glib::idle_add_local_once(move || {
                            if !applied_idle.get() {
                                if let Some(p) = p_weak.upgrade() {
                                    let len = match p.orientation() {
                                        gtk::Orientation::Horizontal => p.width(),
                                        gtk::Orientation::Vertical => p.height(),
                                        _ => p.width(),
                                    };
                                    if len > 0 {
                                        p.set_position((len as f64 * r).round() as i32);
                                        applied_idle.set(true);
                                    }
                                }
                            }
                        });
                    }
                });

                // Also run once via idle immediately in case widget is already mapped
                let paned_init_weak = paned.downgrade();
                let applied_init = std::rc::Rc::clone(&initial_applied);
                glib::idle_add_local_once(move || {
                    if !applied_init.get() {
                        if let Some(p) = paned_init_weak.upgrade() {
                            let len = match p.orientation() {
                                gtk::Orientation::Horizontal => p.width(),
                                gtk::Orientation::Vertical => p.height(),
                                _ => p.width(),
                            };
                            if len > 0 {
                                p.set_position((len as f64 * r).round() as i32);
                                applied_init.set(true);
                            }
                        }
                    }
                });

                // Locate GtkPaned's built-in pointer drag gesture (strictly GtkGestureDrag, excluding subclasses like GtkGesturePan)
                let controllers = paned.observe_controllers();
                let mut drag_opt = None;
                for i in 0..controllers.n_items() {
                    if let Some(item) = controllers.item(i) {
                        if item.type_() == gtk::GestureDrag::static_type() {
                            drag_opt = item.downcast::<gtk::GestureDrag>().ok();
                            break;
                        }
                    }
                }

                let is_dragging = Rc::new(std::cell::Cell::new(false));
                let just_equalized = Rc::new(std::cell::Cell::new(false));
                let split_id = *id;

                let container_weak = container.downgrade();
                let paned_weak = paned.downgrade();
                let model_for_equalize = Rc::clone(model);
                let just_eq_action = Rc::clone(&just_equalized);
                let equalize_action = Rc::new(move || {
                    just_eq_action.set(true);
                    // 1. Capture current live ratios from widgets before equalizing
                    if let Some(container) = container_weak.upgrade() {
                        if let Some(root_widget) = container.first_child() {
                            if let Ok(mut m) = model_for_equalize.try_borrow_mut() {
                                if let Some(root_node) = m.layout.root_mut() {
                                    Self::capture_ratios_recursive(root_node, &root_widget);
                                }
                            }
                        }
                    }

                    // 2. Equalize split cluster in model
                    let cluster_root_id = if let Ok(mut m) = model_for_equalize.try_borrow_mut() {
                        m.layout.equalize_split(split_id)
                    } else {
                        None
                    };

                    // 3. Apply ratios ONLY to the equalized cluster
                    if let Some(root_id) = cluster_root_id {
                        if let Some(container) = container_weak.upgrade() {
                            if let Some(cluster_paned) = Self::find_paned_by_split_id(&container, root_id) {
                                let m = model_for_equalize.borrow();
                                if let Some(node) = m.layout.find_split_node(root_id) {
                                    if let Some(orientation) = m.layout.find_split_orientation(root_id) {
                                        let w = cluster_paned.width();
                                        let h = cluster_paned.height();
                                        Self::apply_cluster_ratios(node, cluster_paned.upcast_ref(), orientation, w, h);
                                        let node_clone = node.clone();
                                        let p_weak = cluster_paned.downgrade();
                                        let just_eq_idle = Rc::clone(&just_eq_action);
                                        glib::idle_add_local_once(move || {
                                            if let Some(p) = p_weak.upgrade() {
                                                let w = p.width();
                                                let h = p.height();
                                                Self::apply_cluster_ratios(&node_clone, p.upcast_ref(), orientation, w, h);
                                            }
                                            just_eq_idle.set(false);
                                        });
                                        return;
                                    }
                                }
                            }
                        }
                    }
                    just_eq_action.set(false);
                });

                let last_click_drag = Rc::new(std::cell::Cell::new(None::<std::time::Instant>));
                let last_click_click = Rc::new(std::cell::Cell::new(None::<std::time::Instant>));

                // 1. Connect to GtkPaned's native drag gesture (which GTK activates only on the separator)
                if let Some(ref drag) = drag_opt {
                    let is_dragging_begin = Rc::clone(&is_dragging);
                    let is_dragging_end = Rc::clone(&is_dragging);
                    let last_click = Rc::clone(&last_click_drag);
                    let eq = Rc::clone(&equalize_action);
                    let paned_for_begin = paned.downgrade();

                    drag.connect_drag_begin(move |gesture, start_x, start_y| {
                        let Some(p) = paned_for_begin.upgrade() else { return; };
                        if !Self::is_point_on_paned_handle(&p, start_x, start_y) {
                            return;
                        }
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
                            gesture.set_state(gtk::EventSequenceState::Denied);
                            eq();
                        }
                    });

                    let model_for_drag = Rc::clone(model);
                    let paned_for_drag = paned.downgrade();
                    let last_click_update = Rc::clone(&last_click_drag);
                    let is_dragging_update = Rc::clone(&is_dragging);
                    drag.connect_drag_update(move |_gesture, offset_x, offset_y| {
                        if !is_dragging_update.get() {
                            return;
                        }
                        if offset_x.hypot(offset_y) > 3.0 {
                            last_click_update.set(None);
                        }
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
                    let last_click_end = Rc::clone(&last_click_drag);
                    let is_dragging_end_check = Rc::clone(&is_dragging);
                    drag.connect_drag_end(move |_gesture, offset_x, offset_y| {
                        if !is_dragging_end_check.get() {
                            return;
                        }
                        is_dragging_end.set(false);
                        if offset_x.hypot(offset_y) > 3.0 {
                            last_click_end.set(None);
                        }
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

                let is_dragging_notify = Rc::clone(&is_dragging);
                let just_eq_notify = Rc::clone(&just_equalized);
                let model_clone_notify = Rc::clone(model);
                paned.connect_notify_local(Some("position"), move |p, _| {
                    if just_eq_notify.get() || !is_dragging_notify.get() {
                        return;
                    }
                    let len = match p.orientation() {
                        gtk::Orientation::Horizontal => p.width(),
                        gtk::Orientation::Vertical => p.height(),
                        _ => p.width(),
                    };
                    if len > 0 && p.position() > 0 {
                        let ratio = (p.position() as f64 / len as f64).clamp(0.05, 0.95);
                        if let Ok(mut m) = model_clone_notify.try_borrow_mut() {
                            m.layout.set_split_ratio(split_id, ratio);
                        }
                    }
                });

                // 2. Attach GestureClick on paned as direct backup
                let click = gtk::GestureClick::new();
                click.set_button(gtk::gdk::BUTTON_PRIMARY);
                click.set_propagation_phase(gtk::PropagationPhase::Capture);
                paned.add_controller(click.clone());
                {
                    let last_click = Rc::clone(&last_click_click);
                    let eq = Rc::clone(&equalize_action);
                    let paned_weak = paned_weak.clone();
                    click.connect_pressed(move |gesture, n_press, x, y| {
                        let Some(p) = paned_weak.upgrade() else { return; };
                        if !Self::is_point_on_paned_handle(&p, x, y) {
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

    fn find_paned_by_split_id(widget: &impl glib::object::IsA<gtk::Widget>, split_id: SplitId) -> Option<gtk::Paned> {
        let w = widget.as_ref();
        let target_name = format!("split-{}", split_id.0);
        if w.widget_name() == target_name {
            if let Some(p) = w.downcast_ref::<gtk::Paned>() {
                return Some(p.clone());
            }
        }
        let mut child = w.first_child();
        while let Some(c) = child {
            if let Some(found) = Self::find_paned_by_split_id(&c, split_id) {
                return Some(found);
            }
            child = c.next_sibling();
        }
        None
    }

    fn apply_cluster_ratios(
        node: &LayoutNode,
        widget: &gtk::Widget,
        target_orientation: SplitOrientation,
        avail_w: i32,
        avail_h: i32,
    ) {
        if let LayoutNode::Split {
            orientation,
            ratio,
            first,
            second,
            ..
        } = node
        {
            if *orientation != target_orientation {
                return;
            }
            if let Some(paned) = widget.downcast_ref::<gtk::Paned>() {
                let is_horiz = *orientation == SplitOrientation::Horizontal;
                let len = match orientation {
                    SplitOrientation::Horizontal => {
                        if paned.width() > 0 { paned.width() } else { avail_w }
                    }
                    SplitOrientation::Vertical => {
                        if paned.height() > 0 { paned.height() } else { avail_h }
                    }
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
                    Self::apply_cluster_ratios(first, &c1, target_orientation, c1_w, c1_h);
                }
                if let Some(c2) = paned.end_child() {
                    Self::apply_cluster_ratios(second, &c2, target_orientation, c2_w, c2_h);
                }
            }
        }
    }

    pub fn equalize_split(&self, split_id: SplitId) {
        self.capture_layout_ratios();
        let cluster_root_id = if let Ok(mut m) = self.model.try_borrow_mut() {
            m.layout.equalize_split(split_id)
        } else {
            None
        };
        let Some(root_id) = cluster_root_id else { return; };
        if let Some(paned) = Self::find_paned_by_split_id(&self.container, root_id) {
            let m = self.model.borrow();
            if let Some(node) = m.layout.find_split_node(root_id) {
                if let Some(orientation) = m.layout.find_split_orientation(root_id) {
                    let w = paned.width();
                    let h = paned.height();
                    Self::apply_cluster_ratios(node, paned.upcast_ref(), orientation, w, h);
                    let node_clone = node.clone();
                    let paned_weak = paned.downgrade();
                    glib::idle_add_local_once(move || {
                        if let Some(p) = paned_weak.upgrade() {
                            let w = p.width();
                            let h = p.height();
                            Self::apply_cluster_ratios(&node_clone, p.upcast_ref(), orientation, w, h);
                        }
                    });
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

    pub fn has_transparent_pane(&self) -> bool {
        self.panes.borrow().values().any(|p| p.is_transparent())
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
                    if item.type_() == gtk::GestureDrag::static_type() {
                        drag_controller = item.downcast::<gtk::GestureDrag>().ok();
                        break;
                    }
                }
            }
            let drag = drag_controller.expect("GtkGestureDrag must exist");

            let window = gtk::Window::new();
            window.set_default_size(400, 300);
            window.set_child(Some(session.widget()));
            window.present();

            let ctx = glib::MainContext::default();
            let start = std::time::Instant::now();
            while (paned.width() == 0 || paned.position() <= 0) && start.elapsed() < std::time::Duration::from_millis(500) {
                ctx.iteration(false);
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            for _ in 0..10 {
                ctx.iteration(false);
            }

            // Simulate drag_begin, drag_update, drag_end
            use glib::prelude::*;
            let initial_pos = paned.position() as f64;
            drag.emit_by_name::<()>("drag-begin", &[&initial_pos, &100.0f64]);
            paned.set_position(paned.position() + 80);
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
            let start = std::time::Instant::now();
            while (paned.width() == 0 || !paned.is_mapped()) && start.elapsed() < std::time::Duration::from_millis(500) {
                ctx.iteration(false);
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
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

            // Ensure position is established
            let pos = paned.position() as f64;
            let center = if pos > 0.0 { pos } else { 200.0 };

            // Hit test: line is drawn at center (width = 1px).
            // Left of line: [center - 6.0 .. center] (6px) picked as separator.
            // Right of line: [center + 1.0 .. center + 7.0] (6px) picked as separator.
            // Outside: < center - 6.0 or > center + 7.0 picked as button.
            for offset in [-6.0, -4.0, -2.0, 0.0, 1.0, 4.0, 7.0] {
                let x = center + offset;
                let picked = paned.pick(x, 150.0, gtk::PickFlags::DEFAULT);
                assert_eq!(
                    picked.map(|w| w.css_name().to_string()),
                    Some("separator".to_string()),
                    "x={} (offset={}) should be picked as separator around center={}",
                    x,
                    offset,
                    center
                );
            }
            // Outside the 6px margin
            assert_eq!(
                paned.pick(center - 7.0, 150.0, gtk::PickFlags::DEFAULT).map(|w| w.css_name().to_string()),
                Some("button".to_string())
            );
            assert_eq!(
                paned.pick(center + 8.0, 150.0, gtk::PickFlags::DEFAULT).map(|w| w.css_name().to_string()),
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
            let paned = session.container.first_child().unwrap().downcast::<gtk::Paned>().unwrap();
            let start = std::time::Instant::now();
            while (paned.width() == 0 || paned.position() <= 0) && start.elapsed() < std::time::Duration::from_millis(500) {
                ctx.iteration(false);
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            for _ in 0..10 {
                ctx.iteration(false);
            }

            if let LayoutNode::Split { ratio, .. } = session.model.borrow().layout.root().unwrap() {
                assert!((*ratio - 0.2).abs() < 0.01, "Initial ratio should be 0.2, got {}", ratio);
            }
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
            window.set_default_size(1000, 600);
            window.set_child(Some(session.widget()));
            window.present();

            let ctx = glib::MainContext::default();
            let paned = session.container.first_child().unwrap().downcast::<gtk::Paned>().unwrap();
            let start = std::time::Instant::now();
            let mut expected_pos = 700;
            while (paned.width() == 0 || (paned.position() - expected_pos).abs() > 2) && start.elapsed() < std::time::Duration::from_millis(1000) {
                if paned.width() > 0 {
                    expected_pos = (paned.width() as f64 * 0.7).round() as i32;
                    session.apply_layout_ratios();
                }
                ctx.iteration(false);
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            for _ in 0..10 {
                ctx.iteration(false);
            }

            assert_eq!(paned.position(), expected_pos);

            // Split Pane 2 vertically
            session.split_pane(PaneId(2), SplitOrientation::Vertical);
            window.present();

            let root_paned = session.container.first_child().unwrap().downcast::<gtk::Paned>().unwrap();
            let start = std::time::Instant::now();
            let mut expected_root_pos = 700;
            while (root_paned.width() == 0 || (root_paned.position() - expected_root_pos).abs() > 2) && start.elapsed() < std::time::Duration::from_millis(1000) {
                if root_paned.width() > 0 {
                    expected_root_pos = (root_paned.width() as f64 * 0.7).round() as i32;
                    session.apply_layout_ratios();
                }
                ctx.iteration(false);
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            for _ in 0..10 {
                ctx.iteration(false);
            }

            let root = session.model.borrow().layout.root().unwrap().clone();
            if let LayoutNode::Split { ratio, .. } = root {
                assert!((ratio - 0.7).abs() < 0.01, "First split ratio should remain 0.7, got {}", ratio);
            } else {
                panic!("Root should be Split");
            }

            assert_eq!(root_paned.position(), expected_root_pos, "Root paned position must be 70%, not reset/equalized");

            session.close();
        });
    }

    #[test]
    fn test_session_view_equalize_split_orthogonal_isolation() {
        crate::ui::window::run_gtk_test(|| {
            let mut model = SessionModel::new(PaneId(1));
            // Vertical split: Top (Pane 1) and Bottom (Pane 2) -> SplitId(1)
            model.split_pane(PaneId(1), SplitOrientation::Vertical).unwrap();
            // Split Top horizontally: Pane 1 and Pane 3 -> SplitId(2)
            model.split_pane(PaneId(1), SplitOrientation::Horizontal).unwrap();
            // Split Bottom horizontally: Pane 2 and Pane 4 -> SplitId(3)
            model.split_pane(PaneId(2), SplitOrientation::Horizontal).unwrap();

            // Mess up ratios
            model.layout.set_split_ratio(SplitId(1), 0.5); // Vertical
            model.layout.set_split_ratio(SplitId(2), 0.2); // Top horizontal
            model.layout.set_split_ratio(SplitId(3), 0.8); // Bottom horizontal

            let session = SessionView::with_model_and_dir(model, None);

            // Equalize ONLY SplitId(2) (Top horizontal)
            session.equalize_split(SplitId(2));

            let m = session.model();
            if let Some(LayoutNode::Split { first, second, .. }) = m.layout.root() {
                // Top horizontal (first) should be equalized to 0.5
                if let LayoutNode::Split { ratio, .. } = &**first {
                    assert!((ratio - 0.5).abs() < 1e-6, "Top horizontal ratio should be 0.5, got {}", ratio);
                } else {
                    panic!("First child must be Split");
                }
                // Bottom horizontal (second) MUST REMAIN 0.8
                if let LayoutNode::Split { ratio, .. } = &**second {
                    assert!((ratio - 0.8).abs() < 1e-6, "Bottom horizontal ratio must remain 0.8, got {}", ratio);
                } else {
                    panic!("Second child must be Split");
                }
            } else {
                panic!("Root must be Split");
            }

            session.close();
        });
    }

    #[test]
    fn test_session_view_balance_layout() {
        crate::ui::window::run_gtk_test(|| {
            let session = SessionView::new();
            session.split_active(SplitOrientation::Horizontal);
            session.split_active(SplitOrientation::Horizontal);
            assert_eq!(session.pane_count(), 3);

            // Mess up ratios in layout
            session.model_mut().layout.set_split_ratio(SplitId(1), 0.8);
            session.model_mut().layout.set_split_ratio(SplitId(2), 0.2);

            // Call balance_layout
            session.balance_layout();

            // Model should have balanced ratios (1/3 and 1/2)
            if let Some(LayoutNode::Split { ratio: r1, second, .. }) = session.model().layout.root() {
                assert!((r1 - (1.0 / 3.0)).abs() < 1e-6, "Root ratio should be 1/3, got {}", r1);
                if let LayoutNode::Split { ratio: r2, .. } = &**second {
                    assert!((r2 - 0.5).abs() < 1e-6, "Second split ratio should be 1/2, got {}", r2);
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
    fn test_equalize_cluster_three_vertical_panes_on_right() {
        crate::ui::window::run_gtk_test(|| {
            let mut model = SessionModel::new(PaneId(1));
            // 1. Split Right (Pane 1 | Pane 2) -> SplitId(1) (Horizontal)
            model.split_pane(PaneId(1), SplitOrientation::Horizontal).unwrap();
            // 2. Split Down on Pane 2 (Pane 2 / Pane 3) -> SplitId(2) (Vertical)
            model.split_pane(PaneId(2), SplitOrientation::Vertical).unwrap();
            // 3. Split Down on Pane 3 (Pane 3 / Pane 4) -> SplitId(3) (Vertical)
            model.split_pane(PaneId(3), SplitOrientation::Vertical).unwrap();

            // Mess up ratios
            model.layout.set_split_ratio(SplitId(1), 0.4); // Left pane takes 40%
            model.layout.set_split_ratio(SplitId(2), 0.6);
            model.layout.set_split_ratio(SplitId(3), 0.2);

            let session = SessionView::with_model_and_dir(model, None);

            // Equalize SplitId(3) (the divider between Pane 3 and Pane 4)
            session.equalize_split(SplitId(3));

            {
                let m = session.model();
                if let Some(LayoutNode::Split { ratio: r_horiz, second, .. }) = m.layout.root() {
                    // Left pane ratio MUST REMAIN 0.4 (untouched!)
                    assert!((r_horiz - 0.4).abs() < 1e-6, "Left pane ratio should remain 0.4, got {}", r_horiz);

                    // In the right column:
                    if let LayoutNode::Split { ratio: r2, second: sub_second, .. } = &**second {
                        // Split 2 must be 1/3 (so Pane 2 gets 1/3)
                        assert!((r2 - (1.0 / 3.0)).abs() < 1e-6, "Split 2 ratio should be 1/3, got {}", r2);

                        if let LayoutNode::Split { ratio: r3, .. } = &**sub_second {
                            // Split 3 must be 1/2 (so Pane 3 and 4 each get 1/3)
                            assert!((r3 - 0.5).abs() < 1e-6, "Split 3 ratio should be 1/2, got {}", r3);
                        } else {
                            panic!("Expected sub_second to be Split");
                        }
                    } else {
                        panic!("Expected second to be Split");
                    }
                } else {
                    panic!("Expected root to be Split");
                }
            }

            // Now mess up again and equalize SplitId(2) (the top divider between Pane 2 and Pane 3)
            session.model_mut().layout.set_split_ratio(SplitId(2), 0.7);
            session.model_mut().layout.set_split_ratio(SplitId(3), 0.3);
            session.equalize_split(SplitId(2));

            let m = session.model();
            if let Some(LayoutNode::Split { ratio: r_horiz, second, .. }) = m.layout.root() {
                assert!((r_horiz - 0.4).abs() < 1e-6, "Left pane ratio should remain 0.4");
                if let LayoutNode::Split { ratio: r2, second: sub_second, .. } = &**second {
                    assert!((r2 - (1.0 / 3.0)).abs() < 1e-6, "Split 2 ratio should be 1/3, got {}", r2);
                    if let LayoutNode::Split { ratio: r3, .. } = &**sub_second {
                        assert!((r3 - 0.5).abs() < 1e-6, "Split 3 ratio should be 1/2, got {}", r3);
                    }
                }
            }

            session.close();
        });
    }

    #[test]
    fn test_equalize_split_nested_de_isolation_in_session_view() {
        crate::ui::window::run_gtk_test(|| {
            let mut model = SessionModel::new(PaneId(1));
            // 1. Split Right (A | B) -> SplitId(1) (Horizontal)
            model.split_pane(PaneId(1), SplitOrientation::Horizontal).unwrap();
            // 2. Split Right (B | C) -> SplitId(2) (Horizontal)
            model.split_pane(PaneId(2), SplitOrientation::Horizontal).unwrap();
            // 3. Split Down (C / D) -> SplitId(3) (Vertical)
            model.split_pane(PaneId(3), SplitOrientation::Vertical).unwrap();
            // 4. Split Right (D | E) -> SplitId(4) (Horizontal)
            model.split_pane(PaneId(4), SplitOrientation::Horizontal).unwrap();

            // Set custom ratios
            model.layout.set_split_ratio(SplitId(1), 0.2); // A gets 20%
            model.layout.set_split_ratio(SplitId(2), 0.7);
            model.layout.set_split_ratio(SplitId(3), 0.6);
            model.layout.set_split_ratio(SplitId(4), 0.1); // D gets 10%

            let session = SessionView::with_model_and_dir(model, None);

            // Double-click D | E separator (SplitId 4)
            session.equalize_split(SplitId(4));

            {
                let m = session.model();
                if let Some(LayoutNode::Split { ratio: r1, second, .. }) = m.layout.root() {
                    // Split 1 MUST REMAIN 0.2
                    assert!((r1 - 0.2).abs() < 1e-6, "Split 1 should remain 0.2, got {}", r1);
                    if let LayoutNode::Split { ratio: r2, second: s2, .. } = &**second {
                        // Split 2 MUST REMAIN 0.7
                        assert!((r2 - 0.7).abs() < 1e-6, "Split 2 should remain 0.7, got {}", r2);
                        if let LayoutNode::Split { ratio: r3, second: s3, .. } = &**s2 {
                            // Split 3 MUST REMAIN 0.6
                            assert!((r3 - 0.6).abs() < 1e-6, "Split 3 should remain 0.6, got {}", r3);
                            if let LayoutNode::Split { ratio: r4, .. } = &**s3 {
                                // Split 4 (D | E) MUST BE EQUALIZED TO 0.5!
                                assert!((r4 - 0.5).abs() < 1e-6, "Split 4 should be 0.5, got {}", r4);
                            } else {
                                panic!("Expected Split 4");
                            }
                        } else {
                            panic!("Expected Split 3");
                        }
                    } else {
                        panic!("Expected Split 2");
                    }
                } else {
                    panic!("Expected root");
                }
            }

            session.close();
        });
    }

    #[test]
    fn test_drag_ab_then_double_click_de_preserves_ab_ratio() {
        crate::ui::window::run_gtk_test(|| {
            crate::ui::window::setup_css();
            let mut model = SessionModel::new(PaneId(1));
            // Split Right -> Split Right -> Split Down -> Split Right
            model.split_pane(PaneId(1), SplitOrientation::Horizontal).unwrap();
            model.split_pane(PaneId(2), SplitOrientation::Horizontal).unwrap();
            model.split_pane(PaneId(3), SplitOrientation::Vertical).unwrap();
            model.split_pane(PaneId(4), SplitOrientation::Horizontal).unwrap();

            let session = SessionView::with_model_and_dir(model, None);
            let window = gtk::Window::new();
            window.set_default_size(1000, 600);
            window.set_child(Some(session.widget()));
            window.present();

            let ctx = glib::MainContext::default();
            // Find Paned 1 (A | B)
            let paned_1 = SessionView::find_paned_by_split_id(&session.container, SplitId(1))
                .expect("Paned 1 must exist");

            let start = std::time::Instant::now();
            while (paned_1.width() == 0 || paned_1.position() <= 0) && start.elapsed() < std::time::Duration::from_millis(500) {
                ctx.iteration(false);
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            for _ in 0..10 {
                ctx.iteration(false);
            }

            // User manually adjusts A and B divider to position 250 (> minimum pane width of 192)
            paned_1.set_position(250);

            for _ in 0..10 {
                ctx.iteration(false);
            }

            let custom_pos = paned_1.position();
            assert!(custom_pos >= 200, "Paned 1 position should be at custom position >= 200, got {}", custom_pos);

            // Now double-click or equalize SplitId(4) (D | E)
            session.equalize_split(SplitId(4));

            for _ in 0..10 {
                ctx.iteration(false);
            }

            // Paned 1 position MUST STILL BE custom_pos, NOT RESET!
            assert_eq!(paned_1.position(), custom_pos, "Paned 1 position must not be reset after equalizing D and E");

            // Paned 4 (D | E) should be equalized (50%)
            let paned_4 = SessionView::find_paned_by_split_id(&session.container, SplitId(4))
                .expect("Paned 4 must exist");
            let p4_len = paned_4.width();
            if p4_len > 0 {
                let p4_ratio = paned_4.position() as f64 / p4_len as f64;
                assert!((p4_ratio - 0.5).abs() < 0.05, "Paned 4 ratio should be ~0.5, got {}", p4_ratio);
            }

            session.close();
        });
    }

    #[test]
    fn test_drag_separator_then_double_click_equalizes() {
        crate::ui::window::run_gtk_test(|| {
            crate::ui::window::setup_css();
            let mut model = SessionModel::new(PaneId(1));
            model.split_pane(PaneId(1), SplitOrientation::Horizontal).unwrap();
            let session = SessionView::with_model_and_dir(model, None);
            let window = gtk::Window::new();
            window.set_default_size(800, 600);
            window.set_child(Some(session.widget()));
            window.present();

            let ctx = glib::MainContext::default();
            let paned = session.container.first_child().unwrap().downcast::<gtk::Paned>().unwrap();
            let start = std::time::Instant::now();
            while (paned.width() == 0 || paned.position() <= 0) && start.elapsed() < std::time::Duration::from_millis(500) {
                ctx.iteration(false);
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            for _ in 0..10 {
                ctx.iteration(false);
            }

            // Find the native GtkGestureDrag controller attached to GtkPaned
            let controllers = paned.observe_controllers();
            let mut native_drag: Option<gtk::GestureDrag> = None;
            for i in 0..controllers.n_items() {
                if let Some(item) = controllers.item(i) {
                    if item.type_() == gtk::GestureDrag::static_type() {
                        native_drag = item.downcast::<gtk::GestureDrag>().ok();
                        break;
                    }
                }
            }
            let drag = native_drag.expect("GtkPaned must have native GtkGestureDrag controller");

            // 1. Drag the separator to 200px (from default ~400px)
            use glib::prelude::*;
            let initial_pos = paned.position() as f64;
            drag.emit_by_name::<()>("drag-begin", &[&initial_pos, &100.0f64]);
            paned.set_position(200);
            drag.emit_by_name::<()>("drag-update", &[&-200.0f64, &0.0f64]);
            drag.emit_by_name::<()>("drag-end", &[&-200.0f64, &0.0f64]);

            for _ in 0..10 {
                ctx.iteration(false);
            }

            // Ratio should now reflect the dragged position (200 / 640 = ~0.31)
            if let LayoutNode::Split { ratio, .. } = session.model.borrow().layout.root().unwrap() {
                assert!((*ratio - 0.5).abs() > 0.1, "Ratio after drag should be different from 0.5, got {}", ratio);
            }

            // 2. Double click the separator at its new position via the native drag gesture
            let cur_pos = paned.position() as f64;
            // First click
            drag.emit_by_name::<()>("drag-begin", &[&cur_pos, &100.0f64]);
            drag.emit_by_name::<()>("drag-end", &[&0.0f64, &0.0f64]);

            // Delay 60ms (well within double-click window [40ms, 450ms])
            std::thread::sleep(std::time::Duration::from_millis(60));

            // Second click
            drag.emit_by_name::<()>("drag-begin", &[&cur_pos, &100.0f64]);
            drag.emit_by_name::<()>("drag-end", &[&0.0f64, &0.0f64]);

            for _ in 0..10 {
                ctx.iteration(false);
            }

            // Ratio must be equalized back to 0.5
            if let LayoutNode::Split { ratio, .. } = session.model.borrow().layout.root().unwrap() {
                assert!((*ratio - 0.5).abs() < 0.01, "Ratio after double click must be equalized to 0.5, got {}", ratio);
            }

            session.close();
        });
    }

    #[test]
    fn test_user_reproduction_equalize_cluster() {
        crate::ui::window::run_gtk_test(|| {
            crate::ui::window::setup_css();
            let model = SessionModel::new(PaneId(1));
            let session = SessionView::with_model_and_dir(model, None);
            let window = gtk::Window::new();
            window.set_default_size(1200, 800);
            window.set_child(Some(session.widget()));
            window.present();

            let ctx = glib::MainContext::default();
            for _ in 0..30 {
                ctx.iteration(false);
            }

            session.split_active(SplitOrientation::Horizontal);
            for _ in 0..30 { ctx.iteration(false); }

            session.split_active(SplitOrientation::Horizontal);
            for _ in 0..30 { ctx.iteration(false); }

            session.split_active(SplitOrientation::Vertical);
            for _ in 0..30 { ctx.iteration(false); }

            session.split_active(SplitOrientation::Vertical);
            for _ in 0..30 { ctx.iteration(false); }

            let paned_4 = SessionView::find_paned_by_split_id(&session.container, SplitId(4)).expect("paned 4 must exist");
            let root_paned = SessionView::find_paned_by_split_id(&session.container, SplitId(1)).unwrap();
            let paned_2 = SessionView::find_paned_by_split_id(&session.container, SplitId(2)).unwrap();
            let paned_3 = SessionView::find_paned_by_split_id(&session.container, SplitId(3)).unwrap();

            let start = std::time::Instant::now();
            while (root_paned.width() == 0 || paned_4.width() == 0 || paned_4.position() <= 0) && start.elapsed() < std::time::Duration::from_millis(500) {
                ctx.iteration(false);
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            for _ in 0..10 { ctx.iteration(false); }

            // Adjust Paned 1 (A | B) to a custom ratio (e.g. 700px, ~58%)
            root_paned.set_position(700);
            for _ in 0..10 { ctx.iteration(false); }

            // Find controllers for all paneds
            let get_drag = |p: &gtk::Paned| -> gtk::GestureDrag {
                let ctrls = p.observe_controllers();
                for i in 0..ctrls.n_items() {
                    if let Some(item) = ctrls.item(i) {
                        if item.type_() == gtk::GestureDrag::static_type() {
                            return item.downcast::<gtk::GestureDrag>().unwrap();
                        }
                    }
                }
                panic!("Paned missing GestureDrag");
            };

            let drag_1 = get_drag(&root_paned);
            let drag_2 = get_drag(&paned_2);
            let drag_3 = get_drag(&paned_3);
            let drag_4 = get_drag(&paned_4);

            use glib::prelude::*;
            // Simulate GTK4 event dispatch hierarchy for a double click at Paned 4's separator
            // Window coords: x ~ 950, y = 600
            // First click
            drag_1.emit_by_name::<()>("drag-begin", &[&950.0f64, &600.0f64]);
            drag_2.emit_by_name::<()>("drag-begin", &[&350.0f64, &600.0f64]);
            drag_3.emit_by_name::<()>("drag-begin", &[&50.0f64, &600.0f64]);
            drag_4.emit_by_name::<()>("drag-begin", &[&50.0f64, &(paned_4.position() as f64)]);

            drag_1.emit_by_name::<()>("drag-end", &[&0.0f64, &0.0f64]);
            drag_2.emit_by_name::<()>("drag-end", &[&0.0f64, &0.0f64]);
            drag_3.emit_by_name::<()>("drag-end", &[&0.0f64, &0.0f64]);
            drag_4.emit_by_name::<()>("drag-end", &[&0.0f64, &0.0f64]);

            std::thread::sleep(std::time::Duration::from_millis(60));

            // Second click (double-click)
            drag_1.emit_by_name::<()>("drag-begin", &[&950.0f64, &600.0f64]);
            drag_2.emit_by_name::<()>("drag-begin", &[&350.0f64, &600.0f64]);
            drag_3.emit_by_name::<()>("drag-begin", &[&50.0f64, &600.0f64]);
            drag_4.emit_by_name::<()>("drag-begin", &[&50.0f64, &(paned_4.position() as f64)]);

            drag_1.emit_by_name::<()>("drag-end", &[&0.0f64, &0.0f64]);
            drag_2.emit_by_name::<()>("drag-end", &[&0.0f64, &0.0f64]);
            drag_3.emit_by_name::<()>("drag-end", &[&0.0f64, &0.0f64]);
            drag_4.emit_by_name::<()>("drag-end", &[&0.0f64, &0.0f64]);

            for _ in 0..30 {
                ctx.iteration(false);
            }

            let (initial_r1, initial_r2) = {
                let m = session.model.borrow();
                let r = m.layout.root().unwrap();
                if let LayoutNode::Split { ratio: r1, second, .. } = r {
                    if let LayoutNode::Split { ratio: r2, .. } = &**second {
                        (*r1, *r2)
                    } else {
                        panic!("Split 2 missing");
                    }
                } else {
                    panic!("Split 1 missing");
                }
            };

            // In the model, Split 1 and Split 2 must have their previous ratios preserved
            let m = session.model.borrow();
            if let Some(LayoutNode::Split { id: s1_id, ratio: r1, second, .. }) = m.layout.root() {
                assert_eq!(*s1_id, SplitId(1));
                assert!((*r1 - initial_r1).abs() < 1e-6, "Split 1 ratio must be preserved! Got {}", r1);
                if let LayoutNode::Split { id: s2_id, ratio: r2, second: s3, .. } = &**second {
                    assert_eq!(*s2_id, SplitId(2));
                    assert!((*r2 - initial_r2).abs() < 1e-6, "Split 2 ratio must be preserved! Got {}", r2);
                    // And the right vertical cluster (Split 3 and 4) MUST BE EQUALIZED!
                    if let LayoutNode::Split { id: s3_id, ratio: r3, second: s4, .. } = &**s3 {
                        assert_eq!(*s3_id, SplitId(3));
                        assert!((*r3 - (1.0 / 3.0)).abs() < 0.02, "Split 3 must be 1/3, got {}", r3);
                        if let LayoutNode::Split { id: s4_id, ratio: r4, .. } = &**s4 {
                            assert_eq!(*s4_id, SplitId(4));
                            assert!((*r4 - 0.5).abs() < 0.02, "Split 4 must be 1/2, got {}", r4);
                        } else {
                            panic!("Expected Split 4");
                        }
                    } else {
                        panic!("Expected Split 3");
                    }
                } else {
                    panic!("Expected Split 2");
                }
            }

            session.close();
        });
    }

    #[test]
    fn test_session_view_sync_input_broadcasting_no_recursion() {
        crate::ui::window::run_gtk_test(|| {
            let session = SessionView::new();
            let p1 = session.active_pane_id().unwrap();
            session.split_active(SplitOrientation::Horizontal);
            let p2 = session.panes.borrow().keys().find(|&&k| k != p1).copied().unwrap();

            session.set_sync_input_enabled(true);
            assert!(session.sync_input_enabled());

            let pane1 = session.panes.borrow().get(&p1).unwrap().clone();
            let pane2 = session.panes.borrow().get(&p2).unwrap().clone();
            assert!(pane1.is_sync_enabled());
            assert!(pane2.is_sync_enabled());

            // Track commits received on pane2
            let p2_commits = Rc::new(RefCell::new(Vec::new()));
            {
                let p2_commits = Rc::clone(&p2_commits);
                pane2.connect_commit(move |_id, text| {
                    p2_commits.borrow_mut().push(text.to_string());
                });
            }

            // Track commits received on pane1
            let p1_commits = Rc::new(RefCell::new(Vec::new()));
            {
                let p1_commits = Rc::clone(&p1_commits);
                pane1.connect_commit(move |_id, text| {
                    p1_commits.borrow_mut().push(text.to_string());
                });
            }

            // Emitting commit on pane1's terminal widget (simulating keyboard input)
            // Without re-entrancy protection, this would cause infinite recursion and stack overflow.
            pane1.terminal().emit_by_name::<()>("commit", &[&"echo synchronized\n", &18u32]);

            // Pane 1's commit callback was invoked once for the simulated user input
            assert_eq!(p1_commits.borrow().len(), 1);
            assert_eq!(p1_commits.borrow()[0], "echo synchronized\n");

            // Pane 2 was fed via feed_child, so its commit callback is suppressed (no reflection loop)
            assert_eq!(p2_commits.borrow().len(), 0);

            // Now test with sync_input_enabled = false
            session.set_sync_input_enabled(false);
            pane1.terminal().emit_by_name::<()>("commit", &[&"secret\n", &7u32]);
            assert_eq!(p1_commits.borrow().len(), 2);
            assert_eq!(p2_commits.borrow().len(), 0);

            session.close();
        });
    }
}



