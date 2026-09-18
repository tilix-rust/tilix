use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gio::prelude::*;
use gtk4 as gtk;
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
            apply_dpi_workaround();
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

/// Calculates the adjusted GTK Xft DPI to compensate for HiDPI/double-scaling issues.
///
/// Returns `Some(target_dpi)` if DPI needs adjustment, or `None` if no change should be made.
///
/// Rules:
/// 1. Only applies on X11 environments (`is_x11 == true`).
/// 2. If `GDK_SCALE` is explicitly set to "1", skip (already 1x).
/// 3. If base DPI is already 96 DPI (98304 in 1/1024th units), skip (no double scaling to fix).
/// 4. If `GDK_DPI_SCALE` is set, scale base DPI by `GDK_DPI_SCALE`.
///    If `GDK_DPI_SCALE` is not set but `GDK_SCALE > 1` (e.g. 2) and DPI is elevated,
///    auto-compensate by `1.0 / GDK_SCALE`.
pub fn calculate_target_dpi(
    is_x11: bool,
    gdk_scale: Option<&str>,
    gdk_dpi_scale: Option<&str>,
    current_xft_dpi: i32,
) -> Option<i32> {
    // 1. 当前环境是 X11
    if !is_x11 {
        return None;
    }

    // 2. 设置了 GDK_SCALE 不能为 1 (如果是 1 则已经是 1x，无需缩放字体)
    if gdk_scale == Some("1") {
        return None;
    }

    // 4. GTK 中 96 DPI 对应 96 * 1024 = 98304；<= 0 表示系统默认 96 DPI
    let base_dpi = if current_xft_dpi > 0 {
        current_xft_dpi
    } else {
        98304
    };

    // 如果 dpi 已经是 96，无需重复缩放
    if base_dpi == 98304 {
        return None;
    }

    // 3. 检查 GDK_DPI_SCALE (或 GDK_SCALE > 1 时的自动补偿)
    let dpi_scale: f64 = if let Some(val) = gdk_dpi_scale {
        match val.parse::<f64>() {
            Ok(v) if v > 0.0 => v,
            _ => return None,
        }
    } else if let Some(scale_str) = gdk_scale {
        match scale_str.parse::<f64>() {
            Ok(scale) if scale > 1.0 => 1.0 / scale,
            _ => return None,
        }
    } else {
        return None;
    };

    // 5. 将 dpi * GDK_DPI_SCALE 的值作为目标 DPI
    let target_dpi = (base_dpi as f64 * dpi_scale).round() as i32;
    Some(target_dpi)
}

/// Adjusts GTK Xft DPI on startup to prevent double-scaling under X11 HiDPI environments.
pub fn apply_dpi_workaround() {
    let is_x11 = if let Some(display) = gtk::gdk::Display::default() {
        display.type_().name().contains("X11")
    } else {
        std::env::var("WAYLAND_DISPLAY").is_err() && std::env::var("DISPLAY").is_ok()
    };

    let gdk_scale = std::env::var("GDK_SCALE").ok();
    let gdk_dpi_scale = std::env::var("GDK_DPI_SCALE").ok();

    if let Some(settings) = gtk::Settings::default() {
        let current_dpi = settings.gtk_xft_dpi();
        if let Some(target_dpi) = calculate_target_dpi(
            is_x11,
            gdk_scale.as_deref(),
            gdk_dpi_scale.as_deref(),
            current_dpi,
        ) {
            settings.set_gtk_xft_dpi(target_dpi);
        }
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

    #[test]
    fn test_calculate_target_dpi_wayland_ignored() {
        assert_eq!(
            calculate_target_dpi(false, Some("2"), Some("0.5"), 196608),
            None
        );
    }

    #[test]
    fn test_calculate_target_dpi_scale_one_ignored() {
        assert_eq!(
            calculate_target_dpi(true, Some("1"), Some("0.5"), 196608),
            None
        );
    }

    #[test]
    fn test_calculate_target_dpi_already_96_ignored() {
        // 98304 is 96 DPI in GTK (96 * 1024)
        assert_eq!(
            calculate_target_dpi(true, Some("2"), Some("0.5"), 98304),
            None
        );
        // Default (-1 or 0) also defaults to 96 DPI
        assert_eq!(
            calculate_target_dpi(true, Some("2"), Some("0.5"), -1),
            None
        );
    }

    #[test]
    fn test_calculate_target_dpi_with_dpi_scale() {
        // 192 DPI (196608) * 0.5 = 98304 (96 DPI)
        assert_eq!(
            calculate_target_dpi(true, Some("2"), Some("0.5"), 196608),
            Some(98304)
        );
        // Even if GDK_SCALE is not explicitly set in env, explicit GDK_DPI_SCALE still applies
        assert_eq!(
            calculate_target_dpi(true, None, Some("0.5"), 196608),
            Some(98304)
        );
    }

    #[test]
    fn test_calculate_target_dpi_scale_2_auto_compensation() {
        // GDK_SCALE=2, DPI=192, no GDK_DPI_SCALE provided -> automatically scales by 1/2 = 0.5
        assert_eq!(
            calculate_target_dpi(true, Some("2"), None, 196608),
            Some(98304)
        );
    }
}
