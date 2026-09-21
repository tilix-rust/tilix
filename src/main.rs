fn main() -> glib::ExitCode {
    let app = tilix::app::TilixApplication::new();
    let args: Vec<String> = std::env::args().collect();
    app.run_with_args(&args)
}

