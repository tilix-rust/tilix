use glib::prelude::*;
use gtk4 as gtk;
use gtk::prelude::*;

use crate::model::PaneId;

pub fn setup_pane_drag_source(widget: &impl IsA<gtk::Widget>, pane_id: PaneId) {
    let drag_source = gtk::DragSource::new();
    drag_source.set_actions(gtk::gdk::DragAction::MOVE);
    let id_val = pane_id.0;
    drag_source.connect_prepare(move |_source, _x, _y| {
        Some(gtk::gdk::ContentProvider::for_value(&id_val.to_value()))
    });
    widget.add_controller(drag_source);
}

pub fn setup_pane_drop_target<F: Fn(PaneId, PaneId) + 'static>(
    widget: &impl IsA<gtk::Widget>,
    dest_id: PaneId,
    on_swap: F,
) {
    let drop_target = gtk::DropTarget::new(glib::Type::U64, gtk::gdk::DragAction::MOVE);
    drop_target.connect_drop(move |_target, value, _x, _y| {
        if let Ok(src_raw) = value.get::<u64>() {
            let src_id = PaneId(src_raw);
            if src_id != dest_id {
                on_swap(src_id, dest_id);
                return true;
            }
        }
        false
    });
    widget.add_controller(drop_target);
}
