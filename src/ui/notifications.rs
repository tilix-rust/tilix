use adw::prelude::*;
use gio::Notification;
use libadwaita as adw;

pub struct NotificationService;

impl NotificationService {
    pub fn notify_bell(app: &adw::Application, pane_title: &str) {
        let notification = Notification::new("Terminal Alert");
        notification.set_body(Some(&format!("Bell received in {}", pane_title)));
        app.send_notification(None, &notification);
    }

    pub fn notify_process_exit(app: &adw::Application, pane_title: &str, exit_code: i32) {
        let notification = Notification::new("Process Completed");
        notification.set_body(Some(&format!(
            "Process completed in {} with exit status {}",
            pane_title, exit_code
        )));
        app.send_notification(None, &notification);
    }
}
