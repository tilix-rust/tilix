# Master Specification: Phase 12 — Dynamic Window Geometry & Default Size Calculation

- **Document ID:** `SPEC-0012`
- **Date:** 2026-09-18
- **Status:** Approved / Ready for Execution
- **Workflow Level:** Medium/Large (Phase 12 of Tilix Rewrite)
- **Preceding Specifications:**
  - [`docs/specs/spec-0001-rust-tilix-foundation-2026-09-15.md`](file:///playground/tilix/docs/specs/spec-0001-rust-tilix-foundation-2026-09-15.md)
  - [`docs/specs/spec-0002-sessions-sync-profiles-2026-09-15.md`](file:///playground/tilix/docs/specs/spec-0002-sessions-sync-profiles-2026-09-15.md)
  - [`docs/specs/spec-0003-wayland-quake-osc-dnd-2026-09-15.md`](file:///playground/tilix/docs/specs/spec-0003-wayland-quake-osc-dnd-2026-09-15.md)
  - [`docs/specs/spec-0004-packaging-distribution-polish-2026-09-16.md`](file:///playground/tilix/docs/specs/spec-0004-packaging-distribution-polish-2026-09-16.md)
  - [`docs/specs/spec-0005-window-style-wide-handle-2026-09-17.md`](file:///playground/tilix/docs/specs/spec-0005-window-style-wide-handle-2026-09-17.md)
  - [`docs/specs/spec-0006-pane-toolbar-terminal-title-2026-09-17.md`](file:///playground/tilix/docs/specs/spec-0006-pane-toolbar-terminal-title-2026-09-17.md)
  - [`docs/specs/spec-0007-custom-keybindings-manager-2026-09-17.md`](file:///playground/tilix/docs/specs/spec-0007-custom-keybindings-manager-2026-09-17.md)
  - [`docs/specs/spec-0008-pane-dnd-docking-detach-2026-09-17.md`](file:///playground/tilix/docs/specs/spec-0008-pane-dnd-docking-detach-2026-09-17.md)
  - [`docs/specs/spec-0009-profile-customization-complete-2026-09-17.md`](file:///playground/tilix/docs/specs/spec-0009-profile-customization-complete-2026-09-17.md)
  - [`docs/specs/spec-0010-profile-preferences-ui-parity-2026-09-17.md`](file:///playground/tilix/docs/specs/spec-0010-profile-preferences-ui-parity-2026-09-17.md)
  - [`docs/specs/spec-0011-session-and-application-title-options-2026-09-18.md`](file:///playground/tilix/docs/specs/spec-0011-session-and-application-title-options-2026-09-18.md)
- **Target Location:** `/playground/tilix`

---

## 1. Context & Motivation

In previous phases, Tilix Rust implemented the foundational terminal multiplexing engine, session management, profiles, themes, preferences UI parity, and hierarchical title evaluation. 

In `src/model/profile.rs`, `Profile` defines two geometry preferences:
- `pub default_size_columns: u32` (default: 80)
- `pub default_size_rows: u32` (default: 24)

These preferences are exposed to the user in the Preferences dialog under General > "Initial terminal size" (`src/ui/preferences.rs`). However, investigation revealed that `TilixWindow::new_empty` in `src/ui/window.rs` hardcodes the window initial size:
```rust
window.set_default_size(900, 600);
```
Consequently, `default_size_columns` and `default_size_rows` are completely ignored when creating windows.

On high-DPI displays (e.g., 196 DPI with `GDK_SCALE=1` or 4K screens) with a 10pt monospace font, a character cell is ~16px wide by ~35px high. In a fixed 900x600 window (minus headerbar, tabbar, pane header, and scrollbar), only ~57 columns by ~16 rows fit. The user expects an initial window that accommodates the configured 80x24 terminal grid. Conversely, on low-DPI displays with small fonts, 900x600 may leave excessive empty space or oversized dimensions.

Phase 12 resolves this issue by introducing dynamic, profile-driven window geometry calculation. The window dimensions are computed from the profile's character grid dimensions, font metrics, cell scaling factors, scrollbar settings, and window chrome decorations, with graceful headless fallbacks and screen clamping against target monitor geometry.

---

## 2. Amended Documents & Scope

- **Amended Document:** [`docs/architecture/overview.md`](file:///playground/tilix/docs/architecture/overview.md)
- **Status of Living Architecture:** Active (Version bumped to 0.12.0 for Phase 12)

### 2.1 Affected Scope
- `src/ui/geometry.rs` (New Module):
  - Pure calculation and measurement functions for window dimensions.
  - VTE font cell measurement (`measure_cell_size`).
  - Terminal grid calculation (`cols * cell_w + scrollbar + margins`, `rows * cell_h + pane_header`).
  - Window chrome summation (HeaderBar, TabBar, borders).
  - Screen boundary clamping (90% monitor bounds, min 300x200).
  - Clean headless fallback to `(900, 600)` when font measurement is unavailable or zero.
- `src/ui/mod.rs`:
  - Declare and re-export `pub mod geometry;`.
- `src/ui/window.rs`:
  - In `TilixWindow::new_empty`, replace hardcoded `(900, 600)` with `calculate_window_size_for_profile`.
  - Add `TilixWindow::new_empty_with_profile(app: &adw::Application, profile: &Profile) -> Self`.
  - Add `TilixWindow::new_with_profile(app: &adw::Application, profile: &Profile) -> Self`.
  - In `detach_drag_to_new_window`, size the new window based on the detached pane's profile.
- `tests/test_phase12_window_geometry.rs` (New Integration Suite):
  - Headless test suite covering font measurement, cell scale proportionality, chrome toggles, simulated HiDPI, monitor clamping, fallbacks, and window instantiation.

### 2.2 Unaffected Scope
- `src/pty/*`: Shell spawning and PTY streams remain unchanged.
- `src/model/layout.rs`: Split tree nodes and docking logic remain unchanged.
- `src/ui/terminal_pane.rs`: VTE terminal widget rendering and overlay logic remain unchanged.
- `src/model/profile.rs` and `src/model/config.rs`: Struct definitions and serialization schemas remain unchanged.

---

## 3. Goals & Non-Goals

### Goals
1. **Accurate Character Cell Measurement:**
   - Accurately measure character cell width and height via `vte::Terminal` configured with the profile's font and cell width/height scale (`char_width()`, `char_height()`).
2. **Comprehensive Geometry Decomposition:**
   - Terminal Grid Width: `cols * cell_width + (if show_scrollbar { 16 } else { 0 }) + (draw_margin * 2)`.
   - Terminal Grid Height: `rows * cell_height + (if pane_header_visible { 36 } else { 0 })`.
   - Window Chrome: HeaderBar (46px if not hidden), TabBar (38px if visible), Window padding (0px).
3. **Screen Boundary Clamping:**
   - Query primary or target `gdk::Monitor` geometry when available.
   - Clamp window dimensions to a maximum of 90% (`0.90`) of the monitor's usable width and height.
   - Clamp window dimensions to a minimum of 300x200 pixels.
4. **Graceful Headless Fallbacks:**
   - If VTE/Pango fails to measure the font, or `char_width <= 0 || char_height <= 0`, fall back cleanly to `(900, 600)` without panics or warnings.
5. **Window Creation Integration:**
   - Seamlessly size new windows in `TilixWindow::new_empty`, `new`, and profile-specific variants.
6. **100% Automated Headless Test Coverage:**
   - Dedicated tests verifying measurement, simulated HiDPI calculation, clamping, chrome options, and window instantiation.

### Non-Goals
- Dynamically resizing existing open windows when profile default column/row preferences change (in upstream Tilix, default size applies exclusively to new window creation).
- Persisting arbitrary user-resized window geometry to config (session layout persistence milestone).
- Querying Wayland compositor desktop workarea margins (clamping to 90% monitor geometry provides standard HIG safety margin across Wayland and X11).

---

## 4. Architecture & Design Decisions

### 4.1 Decision Gating: Facts, Decisions, Assumptions, Deferred Items

- **Facts:**
  - `Profile` has `default_size_columns` (default 80) and `default_size_rows` (default 24).
  - `vte4::TerminalExt::char_width()` and `char_height()` return cell pixel dimensions taking `set_cell_width_scale` and `set_cell_height_scale` into account.
  - In headless CI without a display server, `vte::Terminal` initialized via `gtk::init()` can resolve font metrics for standard monospace fonts (e.g. `Monospace 11` returns 9x20).
  - `gdk::Display::default().and_then(|d| d.monitors().item(0))` retrieves the default monitor.
- **Decisions:**
  - **Modular Architecture (`src/ui/geometry.rs`):** Separate all geometry math into `src/ui/geometry.rs` to maintain clean separation between math/metrics and widget hierarchy.
  - **Explicit Testable Bounds Signature:**
    `calculate_window_size_from_cell_size(profile, app_config, cell_size: Option<(i32, i32)>, monitor_size: Option<(i32, i32)>) -> (i32, i32)`
    allows full testing of simulated HiDPI screens (e.g. 196 DPI) and monitor bounds in headless CI without physical hardware.
  - **Standard Constants:**
    - `DEFAULT_WINDOW_WIDTH: i32 = 900;`
    - `DEFAULT_WINDOW_HEIGHT: i32 = 600;`
    - `MIN_WINDOW_WIDTH: i32 = 300;`
    - `MIN_WINDOW_HEIGHT: i32 = 200;`
    - `DEFAULT_SCROLLBAR_WIDTH: i32 = 16;`
    - `DEFAULT_HEADER_BAR_HEIGHT: i32 = 46;`
    - `DEFAULT_TAB_BAR_HEIGHT: i32 = 38;`
    - `DEFAULT_PANE_HEADER_HEIGHT: i32 = 36;`
    - `MAX_MONITOR_RATIO: f64 = 0.90;`
- **Assumptions:**
  - If `app_config.window_style == WindowStyle::HideToolbar`, HeaderBar is hidden (0px).
  - If `app_config.show_tab_bar == false`, TabBar is hidden (0px).
  - If `app_config.pane_title_style == PaneTitleStyle::None` or `!app_config.pane_title_show_when_single`, pane header is hidden (0px).
- **Deferred Items:**
  - Window state saving/restoring on exit (part of future session serialization milestone).

---

## 5. Window Geometry Mathematics & Measurement Specification

### 5.1 Geometry Formula

Given:
- $C = \max(1, \text{profile.default\_size\_columns})$
- $R = \max(1, \text{profile.default\_size\_rows})$
- $(W_{\text{cell}}, H_{\text{cell}}) = \text{measure\_cell\_size}(\text{profile})$

Terminal Grid:
$$W_{\text{term}} = (C \times W_{\text{cell}}) + W_{\text{scrollbar}} + (2 \times \text{profile.draw\_margin})$$
$$H_{\text{term}} = (R \times H_{\text{cell}}) + H_{\text{pane\_header}}$$

Where:
$$W_{\text{scrollbar}} = \begin{cases} 16 & \text{if } \text{profile.show\_scrollbar} \\ 0 & \text{otherwise} \end{cases}$$
$$H_{\text{pane\_header}} = \begin{cases} 36 & \text{if } \text{pane\_title\_style} \neq \text{None} \land \text{pane\_title\_show\_when\_single} \\ 0 & \text{otherwise} \end{cases}$$

Window Chrome:
$$W_{\text{chrome}} = 0$$
$$H_{\text{chrome}} = H_{\text{header\_bar}} + H_{\text{tab\_bar}}$$

Where:
$$H_{\text{header\_bar}} = \begin{cases} 46 & \text{if } \text{window\_style} \neq \text{HideToolbar} \\ 0 & \text{otherwise} \end{cases}$$
$$H_{\text{tab\_bar}} = \begin{cases} 38 & \text{if } \text{show\_tab\_bar} \\ 0 & \text{otherwise} \end{cases}$$

Total Unclamped Dimensions:
$$W_{\text{raw}} = W_{\text{term}} + W_{\text{chrome}}$$
$$H_{\text{raw}} = H_{\text{term}} + H_{\text{chrome}}$$

Clamping:
If monitor size $(W_{\text{mon}}, H_{\text{mon}})$ is present and valid ($W_{\text{mon}} > 0, H_{\text{mon}} > 0$):
$$W_{\text{max}} = \text{round}(W_{\text{mon}} \times 0.90)$$
$$H_{\text{max}} = \text{round}(H_{\text{mon}} \times 0.90)$$
$$W_{\text{final}} = \text{clamp}(W_{\text{raw}}, \min(300, W_{\text{max}}), W_{\text{max}})$$
$$H_{\text{final}} = \text{clamp}(H_{\text{raw}}, \min(200, H_{\text{max}}), H_{\text{max}})$$
Else:
$$W_{\text{final}} = \max(300, W_{\text{raw}})$$
$$H_{\text{final}} = \max(200, H_{\text{raw}})$$

---

## 6. Implementation Architecture & API Signatures

### 6.1 `src/ui/geometry.rs`
```rust
pub const DEFAULT_WINDOW_WIDTH: i32 = 900;
pub const DEFAULT_WINDOW_HEIGHT: i32 = 600;
pub const MIN_WINDOW_WIDTH: i32 = 300;
pub const MIN_WINDOW_HEIGHT: i32 = 200;
pub const DEFAULT_SCROLLBAR_WIDTH: i32 = 16;
pub const DEFAULT_HEADER_BAR_HEIGHT: i32 = 46;
pub const DEFAULT_TAB_BAR_HEIGHT: i32 = 38;
pub const DEFAULT_PANE_HEADER_HEIGHT: i32 = 36;
pub const MAX_MONITOR_RATIO: f64 = 0.90;

pub fn measure_cell_size(profile: &Profile) -> Option<(i32, i32)>;

pub fn calculate_window_size_from_cell_size(
    profile: &Profile,
    app_config: &AppConfig,
    cell_size: Option<(i32, i32)>,
    monitor_size: Option<(i32, i32)>,
) -> (i32, i32);

pub fn calculate_window_size_with_bounds(
    profile: &Profile,
    app_config: &AppConfig,
    monitor_size: Option<(i32, i32)>,
) -> (i32, i32);

pub fn calculate_window_size_for_profile(
    profile: &Profile,
    app_config: &AppConfig,
    monitor: Option<&gdk::Monitor>,
) -> (i32, i32);
```

### 6.2 `src/ui/window.rs`
```rust
impl TilixWindow {
    pub fn new_empty(app: &adw::Application) -> Self {
        let cfg = crate::model::AppConfig::load();
        let profile = cfg.get_default_profile();
        Self::new_empty_with_profile(app, profile)
    }

    pub fn new_empty_with_profile(app: &adw::Application, profile: &Profile) -> Self {
        let window = adw::ApplicationWindow::new(app);
        let cfg = crate::model::AppConfig::load();
        
        let monitor = gdk::Display::default().and_then(|d| {
            d.monitors().item(0).and_then(|o| o.downcast::<gdk::Monitor>().ok())
        });
        let (width, height) = calculate_window_size_for_profile(profile, &cfg, monitor.as_ref());
        window.set_default_size(width, height);
        window.set_title(Some("Tilix"));
        // ... (remaining widget setup unchanged)
    }

    pub fn new(app: &adw::Application) -> Self {
        let tilix_win = Self::new_empty(app);
        tilix_win.create_tab();
        tilix_win
    }

    pub fn new_with_profile(app: &adw::Application, profile: &Profile) -> Self {
        let tilix_win = Self::new_empty_with_profile(app, profile);
        tilix_win.create_tab();
        tilix_win
    }
}
```

---

## 7. Validation & Verification Plan

1. **Cell Measurement Unit Verification:**
   - Headless VTE measurement returns positive dimensions (`w >= 7`, `h >= 14`) for default monospace font.
   - Proportional cell scale verification (`scale = 1.5` increases cell dimensions by 1.5x).
2. **Formula Decomposition Verification:**
   - Standard 80x24 calculation matches expected pixels.
   - HiDPI 196 DPI test (cell 16x35, 80x24) verifies window size ~1296x960.
   - Custom geometry test (132x43) produces proportionally larger window size.
   - Component toggle tests: disabling scrollbar, headerbar, tabbar, or pane header subtracts exact expected pixel offsets.
3. **Clamping & Fallback Verification:**
   - Small monitor (800x600) clamps large window to 720x540.
   - Min bounds clamp (10x5 grid) ensures dimensions never fall below 300x200.
   - Headless fallback on `char_width <= 0` gracefully returns default `(900, 600)`.
4. **Window Instantiation Integration Verification:**
   - `TilixWindow::new_empty` sets the calculated default size on the underlying `adw::ApplicationWindow`.
   - Full test suite passes without regressions: `cargo test`.

---

## 8. Residual Risks & Technical Debt

- **Display Scaling Reporting on Wayland:** Under Wayland fractional scaling without viewport support, `GdkMonitor` physical pixels might report integer coordinates. Clamping to 90% prevents window overflow under all fractional scaling modes.
- **Font Availability:** If the user specifies an exotic font that is not installed on the system, Pango falls back to the system monospace font; cell measurement continues to succeed.
