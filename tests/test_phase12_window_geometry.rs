#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(deprecated)]

use tilix::{app, model, pty, ui};

use gtk4 as gtk;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use model::config::AppConfig;
use model::profile::Profile;
use model::{PaneTitleStyle, WindowStyle};
use ui::geometry::{
    calculate_window_size_for_profile, calculate_window_size_from_cell_size,
    calculate_window_size_with_bounds, measure_cell_size, DEFAULT_HEADER_BAR_HEIGHT,
    DEFAULT_PANE_HEADER_HEIGHT, DEFAULT_SCROLLBAR_WIDTH, DEFAULT_TAB_BAR_HEIGHT,
    DEFAULT_WINDOW_HEIGHT, DEFAULT_WINDOW_WIDTH, MIN_WINDOW_HEIGHT, MIN_WINDOW_WIDTH,
};
use ui::window::TilixWindow;

fn run_gtk_test<F: FnOnce() + Send + 'static>(f: F) {
    static GTK_TEST_POOL: std::sync::OnceLock<Option<glib::ThreadPool>> = std::sync::OnceLock::new();
    let pool = GTK_TEST_POOL.get_or_init(|| {
        let (init_tx, init_rx) = std::sync::mpsc::sync_channel(1);
        let Ok(pool) = glib::ThreadPool::exclusive(1) else {
            return None;
        };
        if pool
            .push(move || {
                let ok = gtk4::init().is_ok();
                let _ = init_tx.send(ok);
            })
            .is_err()
        {
            return None;
        }
        if init_rx.recv().unwrap_or(false) {
            Some(pool)
        } else {
            None
        }
    });

    if let Some(pool) = pool.as_ref() {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let _ = pool.push(move || {
            f();
            let _ = tx.send(());
        });
        let _ = rx.recv();
    }
}

#[test]
fn test_phase12_measure_cell_size_vte_monospace() {
    run_gtk_test(|| {
        let profile = Profile::default();
        let cell_size = measure_cell_size(&profile);
        assert!(cell_size.is_some(), "Expected cell size to be measured");
        let (w, h) = cell_size.unwrap();
        assert!(w >= 7, "Cell width {} should be >= 7", w);
        assert!(h >= 14, "Cell height {} should be >= 14", h);
    });
}

#[test]
fn test_phase12_cell_scale_proportionality() {
    run_gtk_test(|| {
        let base_profile = Profile {
            cell_width_scale: 1.0,
            cell_height_scale: 1.0,
            ..Default::default()
        };
        let base_size = measure_cell_size(&base_profile).expect("Base cell size");

        let mut scaled_profile = base_profile.clone();
        scaled_profile.cell_width_scale = 1.5;
        scaled_profile.cell_height_scale = 1.5;
        let scaled_size = measure_cell_size(&scaled_profile).expect("Scaled cell size");

        assert!(
            scaled_size.0 > base_size.0,
            "Scaled width {} should be greater than base width {}",
            scaled_size.0,
            base_size.0
        );
        assert!(
            scaled_size.1 > base_size.1,
            "Scaled height {} should be greater than base height {}",
            scaled_size.1,
            base_size.1
        );

        let expected_min_w = (base_size.0 as f64 * 1.4) as i32;
        let expected_min_h = (base_size.1 as f64 * 1.4) as i32;
        assert!(
            scaled_size.0 >= expected_min_w,
            "Scaled width {} should be >= {}",
            scaled_size.0,
            expected_min_w
        );
        assert!(
            scaled_size.1 >= expected_min_h,
            "Scaled height {} should be >= {}",
            scaled_size.1,
            expected_min_h
        );
    });
}

#[test]
fn test_phase12_calculate_window_size_default_80x24() {
    let profile = Profile::default();
    let app_config = AppConfig::default();
    let cell_size = Some((9, 20));
    let (w, h) = calculate_window_size_from_cell_size(&profile, &app_config, cell_size, None);
    // Grid: 80 * 9 + 16 (scrollbar) + 0 = 736
    // Term height: 24 * 20 + 36 (pane header) = 516
    // Chrome height: 46 (headerbar) + 38 (tabbar) = 84
    // Total: 736 x 600
    assert_eq!(w, 736);
    assert_eq!(h, 600);
}

#[test]
fn test_phase12_calculate_window_size_simulated_hidpi_196dpi() {
    let profile = Profile::default();
    let app_config = AppConfig::default();
    // Simulated 196 DPI cell: 16x35, 80x24 grid
    let (w, h) = calculate_window_size_from_cell_size(&profile, &app_config, Some((16, 35)), None);
    // Grid width: 80 * 16 + 16 (scrollbar) = 1296
    // Term height: 24 * 35 + 36 (pane header) = 876
    // Chrome height: 46 (headerbar) + 38 (tabbar) = 84
    // Total: 1296 x 960
    assert_eq!(w, 1296);
    assert_eq!(h, 960);
}

#[test]
fn test_phase12_calculate_window_size_custom_grid() {
    let profile = Profile {
        default_size_columns: 132,
        default_size_rows: 43,
        ..Default::default()
    };
    let app_config = AppConfig::default();
    let (w, h) = calculate_window_size_from_cell_size(&profile, &app_config, Some((9, 20)), None);
    // Grid: 132 * 9 + 16 = 1204
    // Term height: 43 * 20 + 36 = 896
    // Chrome height: 84
    // Total: 1204 x 980
    assert_eq!(w, 1204);
    assert_eq!(h, 980);
}

#[test]
fn test_phase12_chrome_toggles() {
    let base_profile = Profile::default();
    let base_config = AppConfig::default();
    let cell_size = Some((10, 20));
    let (base_w, base_h) =
        calculate_window_size_from_cell_size(&base_profile, &base_config, cell_size, None);

    // 1. Scrollbar hidden
    let mut no_scrollbar_profile = base_profile.clone();
    no_scrollbar_profile.show_scrollbar = false;
    let (w_no_sb, h_no_sb) =
        calculate_window_size_from_cell_size(&no_scrollbar_profile, &base_config, cell_size, None);
    assert_eq!(w_no_sb, base_w - DEFAULT_SCROLLBAR_WIDTH);
    assert_eq!(h_no_sb, base_h);

    // 2. Hide toolbar (headerbar)
    let mut no_tb_config = base_config.clone();
    no_tb_config.window_style = WindowStyle::HideToolbar;
    let (w_no_tb, h_no_tb) =
        calculate_window_size_from_cell_size(&base_profile, &no_tb_config, cell_size, None);
    assert_eq!(w_no_tb, base_w);
    assert_eq!(h_no_tb, base_h - DEFAULT_HEADER_BAR_HEIGHT);

    // 3. Hide tab bar
    let mut no_tabs_config = base_config.clone();
    no_tabs_config.show_tab_bar = false;
    let (w_no_tabs, h_no_tabs) =
        calculate_window_size_from_cell_size(&base_profile, &no_tabs_config, cell_size, None);
    assert_eq!(w_no_tabs, base_w);
    assert_eq!(h_no_tabs, base_h - DEFAULT_TAB_BAR_HEIGHT);

    // 4. Pane title style None
    let mut no_ph_config = base_config.clone();
    no_ph_config.pane_title_style = PaneTitleStyle::None;
    let (w_no_ph, h_no_ph) =
        calculate_window_size_from_cell_size(&base_profile, &no_ph_config, cell_size, None);
    assert_eq!(w_no_ph, base_w);
    assert_eq!(h_no_ph, base_h - DEFAULT_PANE_HEADER_HEIGHT);

    // 5. Pane title show_when_single false
    let mut no_ph_single = base_config.clone();
    no_ph_single.pane_title_show_when_single = false;
    let (w_single, h_single) =
        calculate_window_size_from_cell_size(&base_profile, &no_ph_single, cell_size, None);
    assert_eq!(w_single, base_w);
    assert_eq!(h_single, base_h - DEFAULT_PANE_HEADER_HEIGHT);
}

#[test]
fn test_phase12_screen_clamping_max_bounds() {
    let profile = Profile::default();
    let app_config = AppConfig::default();
    // Terminal grid with cell 16x35 -> raw size 1296x960
    // On 800x600 monitor: 90% is 720x540
    let (w, h) = calculate_window_size_from_cell_size(
        &profile,
        &app_config,
        Some((16, 35)),
        Some((800, 600)),
    );
    assert_eq!(w, 720);
    assert_eq!(h, 540);
}

#[test]
fn test_phase12_screen_clamping_min_bounds() {
    let mut profile = Profile {
        default_size_columns: 10,
        default_size_rows: 5,
        ..Default::default()
    };
    let app_config = AppConfig::default();
    // Raw width: 10 * 9 + 16 = 106 (< 300)
    // Raw height: 5 * 20 + 36 + 84 = 220
    let (w, h) =
        calculate_window_size_from_cell_size(&profile, &app_config, Some((9, 20)), None);
    assert_eq!(w, 300);
    assert_eq!(h, 220);

    // Even smaller grid: 5x2
    profile.default_size_columns = 5;
    profile.default_size_rows = 2;
    // Raw height: 2 * 20 + 36 + 84 = 160 (< 200)
    let (w2, h2) =
        calculate_window_size_from_cell_size(&profile, &app_config, Some((9, 20)), None);
    assert_eq!(w2, 300);
    assert_eq!(h2, 200);
}

#[test]
fn test_phase12_fallback_on_zero_or_negative_dimensions() {
    let profile = Profile::default();
    let app_config = AppConfig::default();

    // None cell size
    let (w1, h1) = calculate_window_size_from_cell_size(&profile, &app_config, None, None);
    assert_eq!((w1, h1), (DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT));

    // Zero width/height
    let (w2, h2) =
        calculate_window_size_from_cell_size(&profile, &app_config, Some((0, 20)), None);
    assert_eq!((w2, h2), (DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT));

    let (w3, h3) =
        calculate_window_size_from_cell_size(&profile, &app_config, Some((10, 0)), None);
    assert_eq!((w3, h3), (DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT));

    // Negative width/height
    let (w4, h4) =
        calculate_window_size_from_cell_size(&profile, &app_config, Some((-5, 20)), None);
    assert_eq!((w4, h4), (DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT));
}

#[test]
fn test_phase12_tilix_window_instantiation_uses_calculated_size() {
    run_gtk_test(|| {
        let app = adw::Application::builder()
            .application_id("com.gexperts.Tilix.TestWindowGeometry")
            .build();

        let window = TilixWindow::new_empty(&app);
        let (w, h) = window.window().default_size();
        assert!(w >= MIN_WINDOW_WIDTH, "Window default width {} should be >= {}", w, MIN_WINDOW_WIDTH);
        assert!(h >= MIN_WINDOW_HEIGHT, "Window default height {} should be >= {}", h, MIN_WINDOW_HEIGHT);

        // Test with a smaller profile (40x15) to verify dynamic calculation without hitting monitor clamping
        let small_profile = Profile {
            default_size_columns: 40,
            default_size_rows: 15,
            ..Default::default()
        };
        let small_win = TilixWindow::new_with_profile(&app, &small_profile);
        let (sw, sh) = small_win.window().default_size();
        assert!(sw < w || w == MIN_WINDOW_WIDTH, "Small profile width {} should be < default {} (or at min)", sw, w);
        assert!(sh < h || h == MIN_WINDOW_HEIGHT, "Small profile height {} should be < default {} (or at min)", sh, h);

        // Also test new_with_profile for a larger profile (120x40), allowing clamping at monitor 90% bounds
        let custom_profile = Profile {
            default_size_columns: 120,
            default_size_rows: 40,
            ..Default::default()
        };
        let profile_win = TilixWindow::new_with_profile(&app, &custom_profile);
        let (pw, ph) = profile_win.window().default_size();
        assert!(pw >= w, "Custom profile width {} should be >= default {}", pw, w);
        assert!(ph >= h, "Custom profile height {} should be >= default {}", ph, h);
    });
}
