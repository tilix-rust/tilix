use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gtk4 as gtk;
use gtk::prelude::*;

use crate::model::{
    ColorScheme, Direction, LayoutNode, LayoutTree, PaneId, Profile, SessionModel,
    SplitOrientation,
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

#[derive(Clone)]
pub struct SessionView {
    container: gtk::Box,
    panes: Rc<RefCell<HashMap<PaneId, TerminalPane>>>,
    model: Rc<RefCell<SessionModel>>,
    action_handler: Rc<RefCell<Option<ActionHandler>>>,
    title_changed_callback: Rc<RefCell<Option<TitleChangedHandler>>>,
}

impl SessionView {
    pub fn new() -> Self {
        Self::with_id(PaneId(1))
    }

    pub fn with_id(initial_pane_id: PaneId) -> Self {
        let model = SessionModel::new(initial_pane_id);
        Self::with_model(model)
    }

    pub fn with_model(model: SessionModel) -> Self {
        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        container.set_vexpand(true);
        container.set_hexpand(true);

        let model = Rc::new(RefCell::new(model));
        let panes = Rc::new(RefCell::new(HashMap::new()));
        let action_handler = Rc::new(RefCell::new(None));
        let title_changed_callback = Rc::new(RefCell::new(None));

        let session = Self {
            container,
            panes,
            model,
            action_handler,
            title_changed_callback,
        };

        let pane_ids = session.model.borrow().layout.panes();
        for id in pane_ids {
            let pane = Self::create_pane(
                id,
                &session.panes,
                &session.model,
                &session.action_handler,
                &session.title_changed_callback,
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
        panes: &Rc<RefCell<HashMap<PaneId, TerminalPane>>>,
        model: &Rc<RefCell<SessionModel>>,
        action_handler: &Rc<RefCell<Option<ActionHandler>>>,
        title_changed_callback: &Rc<RefCell<Option<TitleChangedHandler>>>,
    ) -> TerminalPane {
        let pane = TerminalPane::new(id);
        pane.apply_profile(&Profile::default());

        let handler = Rc::clone(action_handler);
        pane.connect_close(move |p_id| {
            if let Some(cb) = handler.borrow().as_ref() {
                cb(SessionAction::Close(p_id));
            }
        });

        let handler_split = Rc::clone(action_handler);
        pane.connect_split(move |p_id, orient| {
            if let Some(cb) = handler_split.borrow().as_ref() {
                cb(SessionAction::Split(p_id, orient));
            }
        });

        let handler_focus = Rc::clone(action_handler);
        pane.connect_focus(move |p_id| {
            if let Some(cb) = handler_focus.borrow().as_ref() {
                cb(SessionAction::Focus(p_id));
            }
        });

        // Sync toggle callback: update model
        let model_sync = Rc::clone(model);
        pane.connect_sync_toggled(move |p_id, enabled| {
            model_sync.borrow_mut().set_pane_sync_enabled(p_id, enabled);
        });

        // Input broadcasting: commit callback
        let panes_weak = Rc::downgrade(panes);
        let model_commit = Rc::clone(model);
        pane.connect_commit(move |sender_id, text| {
            let Some(panes_rc) = panes_weak.upgrade() else { return; };
            let model = model_commit.borrow();
            if !model.sync_input_enabled {
                return;
            }
            let panes = panes_rc.borrow();
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
            if model_title.borrow().active_pane == Some(p_id) {
                if let Some(ref cb) = *title_cb.borrow() {
                    cb(title);
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
            self.model.borrow_mut().active_pane = Some(id);
            for (pane_id, pane) in panes.iter() {
                pane.set_active(*pane_id == id);
            }
            if let Some(pane) = panes.get(&id) {
                if !pane.terminal().has_focus() {
                    pane.grab_focus();
                }
                let title = pane.title();
                if let Some(ref cb) = *self.title_changed_callback.borrow() {
                    cb(&title);
                }
            }
        }
    }

    pub fn split_active(&self, orientation: SplitOrientation) {
        let active_pane = self.model.borrow().active_pane;
        let target_pane = match active_pane {
            Some(id) if self.panes.borrow().contains_key(&id) => id,
            _ => {
                if let Some(&first_id) = self.panes.borrow().keys().next() {
                    self.model.borrow_mut().active_pane = Some(first_id);
                    first_id
                } else {
                    return;
                }
            }
        };

        self.split_pane(target_pane, orientation);
    }

    pub fn split_pane(&self, target: PaneId, orientation: SplitOrientation) {
        let res = self.model.borrow_mut().split_pane(target, orientation);
        if let Ok(new_id) = res {
            let new_pane = Self::create_pane(
                new_id,
                &self.panes,
                &self.model,
                &self.action_handler,
                &self.title_changed_callback,
            );
            self.panes.borrow_mut().insert(new_id, new_pane);
            self.rebuild_projection();
        }
    }

    pub fn close_active(&self) {
        let active_id = self.model.borrow().active_pane;
        if let Some(id) = active_id {
            self.close_pane(id);
        }
    }

    pub fn close_pane(&self, id: PaneId) {
        if !self.panes.borrow().contains_key(&id) {
            return;
        }

        if self.model.borrow_mut().close_pane(id).is_ok() {
            if let Some(pane) = self.panes.borrow_mut().remove(&id) {
                let w = pane.widget();
                if w.parent().is_some() {
                    w.unparent();
                }
            }
            self.rebuild_projection();
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

    fn build_node(
        node: &LayoutNode,
        panes: &HashMap<PaneId, TerminalPane>,
        model: &Rc<RefCell<SessionModel>>,
    ) -> gtk::Widget {
        match node {
            LayoutNode::Leaf(id) => {
                if let Some(pane) = panes.get(id) {
                    let w = pane.widget();
                    if w.parent().is_some() {
                        w.unparent();
                    }
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
                paned.set_wide_handle(true);
                paned.set_shrink_start_child(false);
                paned.set_shrink_end_child(false);
                paned.set_resize_start_child(true);
                paned.set_resize_end_child(true);

                let first_widget = Self::build_node(first, panes, model);
                let second_widget = Self::build_node(second, panes, model);

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
                        model_clone.borrow_mut().layout.set_split_ratio(split_id, ratio);
                    }
                });

                paned.upcast()
            }
        }
    }

    fn rebuild_projection(&self) {
        let panes = self.panes.borrow();
        for pane in panes.values() {
            let w = pane.widget();
            if w.parent().is_some() {
                w.unparent();
            }
        }

        while let Some(child) = self.container.first_child() {
            child.unparent();
        }

        let model = self.model.borrow();
        let Some(root_node) = model.layout.root() else {
            return;
        };

        let root_widget = Self::build_node(root_node, &panes, &self.model);
        root_widget.set_vexpand(true);
        root_widget.set_hexpand(true);
        self.container.append(&root_widget);

        let active_id = model.active_pane;
        drop(model);
        drop(panes);

        if let Some(id) = active_id {
            self.set_active_pane(id);
        }
    }
}

impl Default for SessionView {
    fn default() -> Self {
        Self::new()
    }
}
