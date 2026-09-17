use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;

use crate::model::{
    AppConfig, ColorScheme, Direction, LayoutNode, LayoutTree, PaneId, PaneTitleStyle, Profile,
    SessionModel, SplitOrientation,
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

#[derive(Clone)]
pub struct SessionView {
    container: gtk::Box,
    panes: Rc<RefCell<HashMap<PaneId, TerminalPane>>>,
    model: Rc<RefCell<SessionModel>>,
    action_handler: Rc<RefCell<Option<ActionHandler>>>,
    title_changed_callback: Rc<RefCell<Option<TitleChangedHandler>>>,
    swap_handler: Rc<RefCell<Option<SwapHandler>>>,
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

    pub fn with_model_and_dir(model: SessionModel, dir: Option<&Path>) -> Self {
        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        container.set_vexpand(true);
        container.set_hexpand(true);

        let model = Rc::new(RefCell::new(model));
        let panes = Rc::new(RefCell::new(HashMap::new()));
        let action_handler = Rc::new(RefCell::new(None));
        let title_changed_callback = Rc::new(RefCell::new(None));
        let swap_handler = Rc::new(RefCell::new(None));
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
            swap_handler,
            use_wide_handle,
            pane_title_style,
            pane_title_show_when_single,
        };

        let panes_c = Rc::clone(&session.panes);
        let model_c = Rc::clone(&session.model);
        let container_c = session.container.clone();
        let use_wide_handle_c = Rc::clone(&session.use_wide_handle);
        *session.swap_handler.borrow_mut() = Some(Box::new(move |src, dest| {
            let swapped = model_c.borrow_mut().layout.swap_panes(src, dest).is_ok();
            if swapped {
                Self::rebuild_projection_with(&container_c, &panes_c, &model_c, *use_wide_handle_c.borrow());
            }
        }));

        let pane_ids = session.model.borrow().layout.panes();
        for id in pane_ids {
            let pane = Self::create_pane(
                id,
                dir,
                &session.panes,
                &session.model,
                &session.action_handler,
                &session.title_changed_callback,
                &session.swap_handler,
            );
            session.panes.borrow_mut().insert(id, pane);
        }

        session.rebuild_projection();
        session
    }

    pub fn set_action_handler<F: Fn(SessionAction) + 'static>(&self, f: F) {
        *self.action_handler.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_title_changed<F: Fn(&str) + 'static>(&self, f: F) {
        *self.title_changed_callback.borrow_mut() = Some(Box::new(f));
    }

    fn create_pane(
        id: PaneId,
        initial_directory: Option<&Path>,
        panes: &Rc<RefCell<HashMap<PaneId, TerminalPane>>>,
        model: &Rc<RefCell<SessionModel>>,
        action_handler: &Rc<RefCell<Option<ActionHandler>>>,
        title_changed_callback: &Rc<RefCell<Option<TitleChangedHandler>>>,
        swap_handler: &Rc<RefCell<Option<SwapHandler>>>,
    ) -> TerminalPane {
        let pane = TerminalPane::new(id, initial_directory);
        let cfg = crate::model::AppConfig::load();
        pane.apply_profile(&cfg.default_profile);

        pane.setup_drag_source();
        let swap_cb = Rc::clone(swap_handler);
        pane.setup_drop_target(move |src, dest| {
            if let Ok(cb_ref) = swap_cb.try_borrow() {
                if let Some(ref cb) = *cb_ref {
                    cb(src, dest);
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
            &self.swap_handler,
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
        let swapped = self.model.borrow_mut().layout.swap_panes(a, b).is_ok();
        if swapped {
            self.rebuild_projection();
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
        let res = self.model.borrow_mut().split_pane(target, orientation);
        if let Ok(new_id) = res {
            let new_pane = Self::create_pane(
                new_id,
                dir,
                &self.panes,
                &self.model,
                &self.action_handler,
                &self.title_changed_callback,
                &self.swap_handler,
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

    fn build_node(
        node: &LayoutNode,
        panes: &HashMap<PaneId, TerminalPane>,
        model: &Rc<RefCell<SessionModel>>,
        wide_handle: bool,
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
                let paned = gtk::Paned::new(gtk_orientation);
                paned.set_wide_handle(wide_handle);
                paned.set_shrink_start_child(false);
                paned.set_shrink_end_child(false);
                paned.set_resize_start_child(true);
                paned.set_resize_end_child(true);

                let first_widget = Self::build_node(first, panes, model, wide_handle);
                let second_widget = Self::build_node(second, panes, model, wide_handle);

                paned.set_start_child(Some(&first_widget));
                paned.set_end_child(Some(&second_widget));

                let r = *ratio;
                paned.connect_realize(move |p| {
                    let len = match p.orientation() {
                        gtk::Orientation::Horizontal => p.width(),
                        gtk::Orientation::Vertical => p.height(),
                        _ => p.width(),
                    };
                    if len > 0 {
                        p.set_position((len as f64 * r).round() as i32);
                    }
                });

                let split_id = *id;
                let model_clone = Rc::clone(model);
                paned.connect_notify_local(Some("position"), move |p, _| {
                    let len = match p.orientation() {
                        gtk::Orientation::Horizontal => p.width(),
                        gtk::Orientation::Vertical => p.height(),
                        _ => p.width(),
                    };
                    if len > 0 {
                        let ratio = (p.position() as f64 / len as f64).clamp(0.05, 0.95);
                        if let Ok(mut m) = model_clone.try_borrow_mut() {
                            m.layout.set_split_ratio(split_id, ratio);
                        }
                    }
                });

                paned.upcast()
            }
        }
    }

    fn rebuild_projection_with(
        container: &gtk::Box,
        panes: &Rc<RefCell<HashMap<PaneId, TerminalPane>>>,
        model: &Rc<RefCell<SessionModel>>,
        wide_handle: bool,
    ) {
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

        let root_widget = Self::build_node(root_node, &panes_b, model, wide_handle);
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

    fn rebuild_projection(&self) {
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
}
