use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gio::prelude::*;
use libadwaita as adw;

use crate::ui::preferences::TilixPreferencesWindow;
use crate::ui::quake::TilixQuakeWindow;
use crate::ui::window::TilixWindow;

pub const APP_ID: &str = "com.github.tilix_rust";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliAction {
    NewWindow,
    QuakeShow,
    QuakeToggle,
    Preferences,
    Help,
    Version,
}

pub fn parse_cli_args<I: IntoIterator<Item = S>, S: AsRef<str>>(args: I) -> CliAction {
    let items: Vec<String> = args.into_iter().map(|s| s.as_ref().to_string()).collect();
    for arg in &items {
        match arg.as_str() {
            "--quake-toggle" => return CliAction::QuakeToggle,
            "--quake" => return CliAction::QuakeShow,
            "--preferences" | "-p" => return CliAction::Preferences,
            "--help" | "-h" => return CliAction::Help,
            "--version" | "-v" => return CliAction::Version,
            _ => {}
        }
    }
    CliAction::NewWindow
}

pub struct TilixApplication {
    app: adw::Application,
    quake_window: Rc<RefCell<Option<TilixQuakeWindow>>>,
}

impl TilixApplication {
    pub fn new() -> Self {
        let app = adw::Application::builder()
            .application_id(APP_ID)
            .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
            .build();

        let quake_window: Rc<RefCell<Option<TilixQuakeWindow>>> = Rc::new(RefCell::new(None));

        app.connect_startup(|app| {
            crate::ui::window::setup_css();
            crate::ui::window::setup_accels(app);
        });

        app.connect_activate(|app| {
            let win = TilixWindow::new(app);
            win.present();
        });

        {
            let quake_win = Rc::clone(&quake_window);
            app.connect_command_line(move |app, cmdline| {
                let args: Vec<String> = cmdline
                    .arguments()
                    .iter()
                    .map(|os| os.to_string_lossy().into_owned())
                    .collect();

                let action = parse_cli_args(&args);
                match action {
                    CliAction::Help => {
                        println!("Usage: tilix [OPTIONS]");
                        println!("  --quake           Launch or present Quake window");
                        println!("  --quake-toggle    Toggle Quake window visibility");
                        println!("  --preferences     Open Preferences dialog");
                        println!("  -h, --help        Show help options");
                        println!("  -v, --version     Show version information");
                    }
                    CliAction::Version => {
                        println!("tilix 0.1.0");
                    }
                    CliAction::QuakeShow => {
                        let mut qw = quake_win.borrow_mut();
                        if qw.is_none() {
                            *qw = Some(TilixQuakeWindow::new(app));
                        }
                        if let Some(ref q) = *qw {
                            q.present();
                        }
                    }
                    CliAction::QuakeToggle => {
                        let mut qw = quake_win.borrow_mut();
                        if qw.is_none() {
                            *qw = Some(TilixQuakeWindow::new(app));
                        }
                        if let Some(ref q) = *qw {
                            q.toggle_visibility();
                        }
                    }
                    CliAction::Preferences => {
                        let on_change = move |profile: &crate::model::Profile| {
                            crate::ui::window::apply_profile_to_all_sessions(profile);
                        };
                        let active_win = app.active_window();
                        let pref = TilixPreferencesWindow::new(active_win.as_ref(), on_change);
                        pref.window().set_application(Some(app));
                        pref.present();
                    }
                    CliAction::NewWindow => {
                        let win = TilixWindow::new(app);
                        win.present();
                    }
                }
                glib::ExitCode::SUCCESS
            });
        }

        Self { app, quake_window }
    }

    pub fn quake_window(&self) -> Rc<RefCell<Option<TilixQuakeWindow>>> {
        Rc::clone(&self.quake_window)
    }

    pub fn run_with_args<S: AsRef<str>>(&self, args: &[S]) -> glib::ExitCode {
        self.app.run_with_args(args)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cli_args_empty_defaults_to_new_window() {
        assert_eq!(parse_cli_args(Vec::<&str>::new()), CliAction::NewWindow);
        assert_eq!(parse_cli_args(["tilix"]), CliAction::NewWindow);
    }

    #[test]
    fn test_parse_cli_args_quake() {
        assert_eq!(parse_cli_args(["--quake"]), CliAction::QuakeShow);
        assert_eq!(parse_cli_args(["tilix", "--quake"]), CliAction::QuakeShow);
    }

    #[test]
    fn test_parse_cli_args_quake_toggle() {
        assert_eq!(parse_cli_args(["--quake-toggle"]), CliAction::QuakeToggle);
        assert_eq!(parse_cli_args(["tilix", "--quake-toggle"]), CliAction::QuakeToggle);
    }

    #[test]
    fn test_parse_cli_args_preferences() {
        assert_eq!(parse_cli_args(["--preferences"]), CliAction::Preferences);
        assert_eq!(parse_cli_args(["tilix", "-p"]), CliAction::Preferences);
    }
}
