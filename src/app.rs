use adw::prelude::*;
use libadwaita as adw;

use crate::ui::window::TilixWindow;

pub const APP_ID: &str = "com.github.tilix_rust";

pub struct TilixApplication {
    app: adw::Application,
}

impl TilixApplication {
    pub fn new() -> Self {
        let app = adw::Application::builder()
            .application_id(APP_ID)
            .build();

        app.connect_startup(|app| {
            crate::ui::window::setup_css();
            crate::ui::window::setup_accels(app);
        });

        app.connect_activate(|app| {
            let win = TilixWindow::new(app);
            win.present();
        });

        Self { app }
    }

    pub fn run(&self) -> glib::ExitCode {
        self.app.run()
    }
}

impl Default for TilixApplication {
    fn default() -> Self {
        Self::new()
    }
}
