pub mod app;
pub mod model;
pub mod pty;
pub mod ui;

fn main() -> glib::ExitCode {
    let app = app::TilixApplication::new();
    let args: Vec<String> = std::env::args().collect();
    app.run_with_args(&args)
}
