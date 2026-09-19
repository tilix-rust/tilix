use gtk4 as gtk;
use gtk4::gdk;
use gtk4::gdk::prelude::*;
use vte4 as vte;
use vte::prelude::*;

use crate::model::config::AppConfig;
use crate::model::profile::Profile;
use crate::model::{PaneTitleStyle, WindowStyle};

pub const DEFAULT_WINDOW_WIDTH: i32 = 900;
pub const DEFAULT_WINDOW_HEIGHT: i32 = 600;
pub const MIN_WINDOW_WIDTH: i32 = 300;
pub const MIN_WINDOW_HEIGHT: i32 = 200;
pub const DEFAULT_SCROLLBAR_WIDTH: i32 = 16;
pub const DEFAULT_HEADER_BAR_HEIGHT: i32 = 46;
pub const DEFAULT_TAB_BAR_HEIGHT: i32 = 38;
pub const DEFAULT_PANE_HEADER_HEIGHT: i32 = 36;
pub const COMPACT_HEADER_BAR_HEIGHT: i32 = 28;
pub const COMPACT_TAB_BAR_HEIGHT: i32 = 22;
pub const COMPACT_PANE_HEADER_HEIGHT: i32 = 22;
pub const MAX_MONITOR_RATIO: f64 = 0.90;

/// Measures the character cell dimensions for the given profile using a headless VTE terminal instance.
/// Returns `Some((char_width, char_height))` if both dimensions are positive (> 0), or `None` otherwise.
pub fn measure_cell_size(profile: &Profile) -> Option<(i32, i32)> {
    let term = vte::Terminal::new();
    if profile.use_system_font {
        let font_desc = gtk::pango::FontDescription::from_string("Monospace 11");
        term.set_font(Some(&font_desc));
    } else if let Some(ref font_name) = profile.font {
        let font_desc = gtk::pango::FontDescription::from_string(font_name);
        term.set_font(Some(&font_desc));
    }

    let w_scale = profile.cell_width_scale.clamp(1.0, 2.0);
    let h_scale = profile.cell_height_scale.clamp(1.0, 2.0);
    term.set_cell_width_scale(w_scale);
    term.set_cell_height_scale(h_scale);

    let char_w = term.char_width() as i32;
    let char_h = term.char_height() as i32;
    if char_w > 0 && char_h > 0 {
        Some((char_w, char_h))
    } else {
        None
    }
}

/// Calculates the target window dimensions from explicit cell dimensions and optional monitor bounds.
/// If cell dimensions are invalid or missing, falls back cleanly to `(DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT)`.
/// Clamps dimensions to `MIN_WINDOW_WIDTH` x `MIN_WINDOW_HEIGHT`, and to 90% of monitor dimensions if provided.
pub fn calculate_window_size_from_cell_size(
    profile: &Profile,
    app_config: &AppConfig,
    cell_size: Option<(i32, i32)>,
    monitor_size: Option<(i32, i32)>,
) -> (i32, i32) {
    let (raw_w, raw_h) = match cell_size {
        Some((w, h)) if w > 0 && h > 0 => {
            let cols = profile.default_size_columns.max(1) as i32;
            let rows = profile.default_size_rows.max(1) as i32;

            let scrollbar_w = if profile.show_scrollbar {
                DEFAULT_SCROLLBAR_WIDTH
            } else {
                0
            };
            let margin_w = (profile.draw_margin as i32) * 2;

            let (header_bar_base, tab_bar_base, pane_header_base) = if app_config.compact_mode {
                (COMPACT_HEADER_BAR_HEIGHT, COMPACT_TAB_BAR_HEIGHT, COMPACT_PANE_HEADER_HEIGHT)
            } else {
                (DEFAULT_HEADER_BAR_HEIGHT, DEFAULT_TAB_BAR_HEIGHT, DEFAULT_PANE_HEADER_HEIGHT)
            };

            let pane_header_h = if app_config.pane_title_style != PaneTitleStyle::None
                && app_config.pane_title_show_when_single
            {
                pane_header_base
            } else {
                0
            };

            let header_bar_h = if app_config.window_style != WindowStyle::HideToolbar {
                header_bar_base
            } else {
                0
            };

            let tab_bar_h = if app_config.show_tab_bar {
                tab_bar_base
            } else {
                0
            };

            let term_w = (cols * w) + scrollbar_w + margin_w;
            let term_h = (rows * h) + pane_header_h;

            let chrome_w = 0;
            let chrome_h = header_bar_h + tab_bar_h;

            (term_w + chrome_w, term_h + chrome_h)
        }
        _ => (DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT),
    };

    if let Some((mon_w, mon_h)) = monitor_size {
        if mon_w > 0 && mon_h > 0 {
            let max_w = (mon_w as f64 * MAX_MONITOR_RATIO).round() as i32;
            let max_h = (mon_h as f64 * MAX_MONITOR_RATIO).round() as i32;

            let min_w = MIN_WINDOW_WIDTH.min(max_w);
            let min_h = MIN_WINDOW_HEIGHT.min(max_h);

            let clamped_w = raw_w.clamp(min_w, max_w);
            let clamped_h = raw_h.clamp(min_h, max_h);

            return (clamped_w, clamped_h);
        }
    }

    let clamped_w = raw_w.max(MIN_WINDOW_WIDTH);
    let clamped_h = raw_h.max(MIN_WINDOW_HEIGHT);

    (clamped_w, clamped_h)
}

/// Measures font cell dimensions from the profile and computes window size within optional monitor bounds.
pub fn calculate_window_size_with_bounds(
    profile: &Profile,
    app_config: &AppConfig,
    monitor_size: Option<(i32, i32)>,
) -> (i32, i32) {
    let cell_size = measure_cell_size(profile);
    calculate_window_size_from_cell_size(profile, app_config, cell_size, monitor_size)
}

/// Measures font cell dimensions and calculates window size for the target profile and monitor.
pub fn calculate_window_size_for_profile(
    profile: &Profile,
    app_config: &AppConfig,
    monitor: Option<&gdk::Monitor>,
) -> (i32, i32) {
    let monitor_size = monitor.map(|m| {
        let geom = m.geometry();
        (geom.width(), geom.height())
    });
    calculate_window_size_with_bounds(profile, app_config, monitor_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compact_geometry_constants() {
        assert_eq!(COMPACT_HEADER_BAR_HEIGHT, 28);
        assert_eq!(COMPACT_TAB_BAR_HEIGHT, 22);
        assert_eq!(COMPACT_PANE_HEADER_HEIGHT, 22);
    }

    #[test]
    fn test_calculate_window_size_compact_mode_delta() {
        let profile = Profile::default();
        let normal_cfg = AppConfig {
            compact_mode: false,
            ..Default::default()
        };

        let compact_cfg = AppConfig {
            compact_mode: true,
            ..Default::default()
        };

        let cell_size = Some((9, 20));
        let (nw, nh) = calculate_window_size_from_cell_size(&profile, &normal_cfg, cell_size, None);
        let (cw, ch) = calculate_window_size_from_cell_size(&profile, &compact_cfg, cell_size, None);

        // Width unchanged
        assert_eq!(nw, cw);
        assert_eq!(nw, 80 * 9 + 16);

        // Height delta: (46 - 28) + (38 - 22) + (36 - 22) = 18 + 16 + 14 = 48px
        assert_eq!(nh - ch, 48);
        assert_eq!(nh, 24 * 20 + 36 + 46 + 38); // 600
        assert_eq!(ch, 24 * 20 + 22 + 28 + 22); // 552
    }

    #[test]
    fn test_calculate_window_size_compact_mode_hidden_chrome() {
        let profile = Profile::default();
        let mut normal_cfg = AppConfig::default();
        normal_cfg.window_style = WindowStyle::HideToolbar;
        normal_cfg.show_tab_bar = false;
        normal_cfg.pane_title_style = PaneTitleStyle::None;
        normal_cfg.compact_mode = false;

        let mut compact_cfg = normal_cfg.clone();
        compact_cfg.compact_mode = true;

        let cell_size = Some((9, 20));
        let (nw, nh) = calculate_window_size_from_cell_size(&profile, &normal_cfg, cell_size, None);
        let (cw, ch) = calculate_window_size_from_cell_size(&profile, &compact_cfg, cell_size, None);

        assert_eq!((nw, nh), (cw, ch));
        assert_eq!(nw, 736);
        assert_eq!(nh, 480);
    }
}
