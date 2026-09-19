use std::cell::RefCell;

use glib::prelude::*;
use gtk4 as gtk;
use gtk::prelude::*;

use crate::model::{calculate_dock_position, DockPosition, PaneId};
use crate::ui::terminal_pane::TerminalPane;

#[derive(Clone)]
pub struct ActivePaneDrag {
    pub pane_id: PaneId,
    pub source_session_widget: glib::WeakRef<gtk::Widget>,
}

thread_local! {
    static ACTIVE_PANE_DRAG: RefCell<Option<ActivePaneDrag>> = const { RefCell::new(None) };
}

pub fn get_active_pane_drag() -> Option<ActivePaneDrag> {
    ACTIVE_PANE_DRAG.with(|cell| cell.borrow().clone())
}

pub fn take_active_pane_drag() -> Option<ActivePaneDrag> {
    ACTIVE_PANE_DRAG.with(|cell| cell.borrow_mut().take())
}

pub fn set_active_pane_drag(drag: Option<ActivePaneDrag>) {
    ACTIVE_PANE_DRAG.with(|cell| *cell.borrow_mut() = drag);
}

pub fn clear_active_pane_drag() {
    ACTIVE_PANE_DRAG.with(|cell| *cell.borrow_mut() = None);
}

pub fn setup_pane_drag_source(
    widget: &impl IsA<gtk::Widget>,
    pane: TerminalPane,
    require_alt: bool,
) {
    let drag_source = gtk::DragSource::new();
    drag_source.set_actions(gtk::gdk::DragAction::MOVE);
    let pane_id = pane.pane_id();
    let id_val = pane_id.0;

    if require_alt {
        drag_source.set_propagation_phase(gtk::PropagationPhase::Capture);
        let pane_focus = pane.clone();
        drag_source.connect_begin(move |gesture, seq| {
            let state = gesture.current_event_state();
            let has_mod = state.intersects(
                gtk::gdk::ModifierType::ALT_MASK
                    | gtk::gdk::ModifierType::META_MASK
                    | gtk::gdk::ModifierType::SUPER_MASK,
            );
            if has_mod {
                pane_focus.grab_focus();
                #[allow(deprecated)]
                if let Some(s) = seq {
                    gesture.set_sequence_state(s, gtk::EventSequenceState::Claimed);
                } else {
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                }
            } else {
                #[allow(deprecated)]
                if let Some(s) = seq {
                    gesture.set_sequence_state(s, gtk::EventSequenceState::Denied);
                } else {
                    gesture.set_state(gtk::EventSequenceState::Denied);
                }
            }
        });
    }

    drag_source.connect_prepare(move |source, _x, _y| {
        if require_alt {
            let state = source.current_event_state();
            let has_mod = state.intersects(
                gtk::gdk::ModifierType::ALT_MASK
                    | gtk::gdk::ModifierType::META_MASK
                    | gtk::gdk::ModifierType::SUPER_MASK,
            );
            if !has_mod {
                return None;
            }
        }

        let session_widget = source
            .widget()
            .and_then(|w| crate::ui::window::find_session_widget(&w))
            .map(|w| w.downgrade())
            .unwrap_or_default();

        let active = ActivePaneDrag {
            pane_id,
            source_session_widget: session_widget,
        };
        set_active_pane_drag(Some(active));

        Some(gtk::gdk::ContentProvider::for_value(&id_val.to_value()))
    });

    let pane_widget = pane.widget().clone();
    drag_source.connect_drag_begin(move |source, _drag| {
        let paintable = gtk::WidgetPaintable::new(Some(&pane_widget));
        source.set_icon(Some(&paintable), 0, 0);
    });

    drag_source.connect_drag_cancel(move |_source, _drag, reason| {
        if reason == gtk::gdk::DragCancelReason::NoTarget
            && crate::ui::window::detach_drag_to_new_window()
        {
            return true;
        }
        false
    });

    drag_source.connect_drag_end(move |_source, _drag, _delete_data| {
        clear_active_pane_drag();
    });

    widget.add_controller(drag_source);
}

pub fn setup_pane_drop_target<F: Fn(PaneId, PaneId, DockPosition) + 'static>(
    widget: &impl IsA<gtk::Widget>,
    pane: TerminalPane,
    on_dock: F,
) {
    let drop_target = gtk::DropTarget::new(glib::Type::U64, gtk::gdk::DragAction::MOVE);
    let dest_id = pane.pane_id();

    let pane_motion = pane.clone();
    drop_target.connect_motion(move |_target, x, y| {
        if let Some(drag) = get_active_pane_drag() {
            if drag.pane_id == dest_id {
                pane_motion.hide_drop_indicator();
                return gtk::gdk::DragAction::empty();
            }
        }
        let w = pane_motion.widget().width() as f64;
        let h = pane_motion.widget().height() as f64;
        let pos = calculate_dock_position(x, y, w, h);
        pane_motion.show_drop_indicator(pos);
        gtk::gdk::DragAction::MOVE
    });

    let pane_leave = pane.clone();
    drop_target.connect_leave(move |_target| {
        pane_leave.hide_drop_indicator();
    });

    let pane_drop = pane;
    drop_target.connect_drop(move |_target, value, x, y| {
        pane_drop.hide_drop_indicator();
        if let Ok(src_raw) = value.get::<u64>() {
            let src_id = PaneId(src_raw);
            if src_id != dest_id {
                let w = pane_drop.widget().width() as f64;
                let h = pane_drop.widget().height() as f64;
                let pos = calculate_dock_position(x, y, w, h);
                on_dock(src_id, dest_id, pos);
                return true;
            }
        }
        false
    });

    widget.add_controller(drop_target);
}
