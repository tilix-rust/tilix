use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gtk4 as gtk;
use gtk::prelude::*;

use crate::model::{Direction, LayoutNode, LayoutTree, PaneId, SessionModel, SplitOrientation};
use crate::ui::terminal_pane::TerminalPane;

#[derive(Debug, Clone)]
pub enum SessionAction {
    Split(PaneId, SplitOrientation),
    Close(PaneId),
    Focus(PaneId),
}

pub type ActionHandler = Box<dyn Fn(SessionAction)>;

pub struct SessionView {
    container: gtk::Box,
    panes: HashMap<PaneId, TerminalPane>,
    model: SessionModel,
    action_handler: Rc<RefCell<Option<ActionHandler>>>,
}

impl SessionView {
    pub fn new() -> Self {
        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        container.set_vexpand(true);
        container.set_hexpand(true);

        let initial_pane_id = PaneId(1);
        let model = SessionModel::new(initial_pane_id);
        let panes = HashMap::new();
        let action_handler = Rc::new(RefCell::new(None));

        let mut session = Self {
            container,
            panes,
            model,
            action_handler,
        };

        let initial_pane = session.create_pane(initial_pane_id);
        session.panes.insert(initial_pane_id, initial_pane);
        session.rebuild_projection();

        session
    }

    pub fn set_action_handler<F: Fn(SessionAction) + 'static>(&self, f: F) {
        *self.action_handler.borrow_mut() = Some(Box::new(f));
    }

    fn create_pane(&self, id: PaneId) -> TerminalPane {
        let pane = TerminalPane::new(id);

        let handler = Rc::clone(&self.action_handler);
        pane.connect_close(move |p_id| {
            if let Some(cb) = handler.borrow().as_ref() {
                cb(SessionAction::Close(p_id));
            }
        });

        let handler_split = Rc::clone(&self.action_handler);
        pane.connect_split(move |p_id, orient| {
            if let Some(cb) = handler_split.borrow().as_ref() {
                cb(SessionAction::Split(p_id, orient));
            }
        });

        let handler_focus = Rc::clone(&self.action_handler);
        pane.connect_focus(move |p_id| {
            if let Some(cb) = handler_focus.borrow().as_ref() {
                cb(SessionAction::Focus(p_id));
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

    pub fn panes(&self) -> &HashMap<PaneId, TerminalPane> {
        &self.panes
    }

    pub fn model(&self) -> &SessionModel {
        &self.model
    }

    pub fn model_mut(&mut self) -> &mut SessionModel {
        &mut self.model
    }

    pub fn layout(&self) -> &LayoutTree {
        &self.model.layout
    }

    pub fn is_empty(&self) -> bool {
        self.panes.is_empty()
    }

    pub fn active_pane_id(&self) -> Option<PaneId> {
        self.model.active_pane
    }

    pub fn set_active_pane(&mut self, id: PaneId) {
        if self.panes.contains_key(&id) {
            self.model.active_pane = Some(id);
            for (pane_id, pane) in &self.panes {
                pane.set_active(*pane_id == id);
            }
            if let Some(pane) = self.panes.get(&id) {
                if !pane.terminal().has_focus() {
                    pane.grab_focus();
                }
            }
        }
    }

    pub fn split_active(&mut self, orientation: SplitOrientation) {
        if self.model.active_pane.is_none()
            || !self
                .panes
                .contains_key(&self.model.active_pane.unwrap())
        {
            if let Some(&first_id) = self.panes.keys().next() {
                self.model.active_pane = Some(first_id);
            } else {
                return;
            }
        }

        if let Ok(new_id) = self.model.split_active(orientation) {
            let new_pane = self.create_pane(new_id);
            self.panes.insert(new_id, new_pane);
            self.rebuild_projection();
        }
    }

    pub fn close_active(&mut self) {
        if let Some(id) = self.model.active_pane {
            self.close_pane(id);
        }
    }

    pub fn close_pane(&mut self, id: PaneId) {
        if !self.panes.contains_key(&id) {
            return;
        }

        if self.model.close_pane(id).is_ok() {
            if let Some(pane) = self.panes.remove(&id) {
                let w = pane.widget();
                if w.parent().is_some() {
                    w.unparent();
                }
            }
            self.rebuild_projection();
        }
    }

    pub fn focus_adjacent(&mut self, direction: Direction) {
        if let Some(adjacent) = self.model.focus_adjacent(direction) {
            self.set_active_pane(adjacent);
        }
    }

    pub fn balance_layout(&mut self) {
        self.model.layout.balance();
        self.rebuild_projection();
    }

    fn build_node(node: &LayoutNode, panes: &HashMap<PaneId, TerminalPane>) -> gtk::Widget {
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

                let first_widget = Self::build_node(first, panes);
                let second_widget = Self::build_node(second, panes);

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

                paned.upcast()
            }
        }
    }

    fn rebuild_projection(&mut self) {
        // First unparent all pane widgets from any prior parents to allow clean reparenting
        for pane in self.panes.values() {
            let w = pane.widget();
            if w.parent().is_some() {
                w.unparent();
            }
        }

        // Clear existing children from the container
        while let Some(child) = self.container.first_child() {
            child.unparent();
        }

        let Some(root_node) = self.model.layout.root() else {
            return;
        };

        let root_widget = Self::build_node(root_node, &self.panes);
        root_widget.set_vexpand(true);
        root_widget.set_hexpand(true);
        self.container.append(&root_widget);

        if let Some(active_id) = self.model.active_pane {
            self.set_active_pane(active_id);
        }
    }
}

impl Default for SessionView {
    fn default() -> Self {
        Self::new()
    }
}
