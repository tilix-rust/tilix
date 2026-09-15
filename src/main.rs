pub mod app;
pub mod model;
pub mod pty;
pub mod ui;

fn main() -> glib::ExitCode {
    let app = app::TilixApplication::new();
    app.run()
}
