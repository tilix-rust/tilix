# Delivery Result Record: Phase 12 — Dynamic Window Geometry & Default Size Calculation

- **Document ID:** `RESULT-0012`
- **Task Reference:** [`docs/tasks/task-0012.md`](file:///playground/tilix/docs/tasks/task-0012.md)
- **Spec Reference:** [`docs/specs/spec-0012-dynamic-window-geometry-default-size-2026-09-18.md`](file:///playground/tilix/docs/specs/spec-0012-dynamic-window-geometry-default-size-2026-09-18.md)
- **Living Architecture Reference:** [`docs/architecture/overview.md`](file:///playground/tilix/docs/architecture/overview.md)
- **Date:** 2026-09-18
- **Status:** Completed & Verified (Approved by Review Gate)

---

## 1. Executive Summary

Phase 12 addresses the issue where new Tilix windows opened with an arbitrary fixed size (`900x600`), resulting in only ~57x16 characters on high-DPI displays (e.g. 196 DPI, `GDK_SCALE=1`, 10pt font) and ignoring the profile's configured `default_size_columns` (default 80) and `default_size_rows` (default 24).

The phase introduces a modular, profile-driven window geometry calculation subsystem:

1. **Pure Window Geometry Subsystem (`src/ui/geometry.rs`):**
   - Headless VTE character cell measurement (`measure_cell_size`) extracting exact font cell dimensions (`char_width`, `char_height`) incorporating `cell_width_scale` and `cell_height_scale`.
   - Accurate grid dimension calculation:
     $$W_{\text{term}} = (C \times W_{\text{cell}}) + W_{\text{scrollbar}} + 2 \times \text{draw\_margin}$$
     $$H_{\text{term}} = (R \times H_{\text{cell}}) + H_{\text{pane\_header}}$$
   - Window chrome summation:
     - HeaderBar: 46px (suppressed if `WindowStyle::HideToolbar`).
     - TabBar: 38px (suppressed if `show_tab_bar == false`).
     - Pane Header: 36px (suppressed if `PaneTitleStyle::None` or `!pane_title_show_when_single`).
     - Scrollbar: 16px (suppressed if `show_scrollbar == false`).
   - Screen boundary clamping:
     - Clamps window dimensions to a maximum of 90% (`0.90`) of the target monitor's usable width and height.
     - Clamps window dimensions to a minimum of 300x200 pixels.
   - Robust headless fallback returning safe default dimensions `(900, 600)` whenever font cell dimensions are unmeasurable or non-positive.

2. **Window Creation Integration (`src/ui/window.rs`):**
   - Refactored `TilixWindow::new_empty` to determine initial window dimensions dynamically using the default profile and active target monitor.
   - Provided `TilixWindow::new_empty_with_profile(app, profile)` and `TilixWindow::new_with_profile(app, profile)` to support profile-specific window sizing.
   - Updated tab detachment (`detach_drag_to_new_window`) to size detached windows according to the detached pane's profile.

3. **Living Architecture & Full Regression Testing:**
   - Updated `docs/architecture/overview.md` to version `0.12.0` with Section 24 documenting the Window Geometry & Dynamic Sizing subsystem.
   - Comprehensive headless integration test suite (`tests/test_phase12_window_geometry.rs`) passing 10/10 tests.
   - Full workspace test suite execution passed with 146 unit tests and 0 regressions.

---

## 2. Completed Work & Delivered Files

| File | Status | Description |
| :--- | :--- | :--- |
| [`src/ui/geometry.rs`](file:///playground/tilix/src/ui/geometry.rs) | Created | Pure geometry calculation module providing constants, `measure_cell_size`, `calculate_window_size_from_cell_size`, `calculate_window_size_with_bounds`, and `calculate_window_size_for_profile`. |
| [`src/ui/mod.rs`](file:///playground/tilix/src/ui/mod.rs) | Modified | Declared and re-exported `geometry` module items and public calculation functions. |
| [`src/ui/window.rs`](file:///playground/tilix/src/ui/window.rs) | Modified | Integrated dynamic geometry calculation into `TilixWindow::new_empty`, added profile-specific constructors `new_empty_with_profile` / `new_with_profile`, and updated drag detachment sizing. |
| [`docs/architecture/overview.md`](file:///playground/tilix/docs/architecture/overview.md) | Modified | Bumped living architecture version to 0.12.0 and added Section 24 documenting Window Geometry & Dynamic Sizing. |
| [`docs/specs/spec-0012-dynamic-window-geometry-default-size-2026-09-18.md`](file:///playground/tilix/docs/specs/spec-0012-dynamic-window-geometry-default-size-2026-09-18.md) | Created | Master architectural specification for Phase 12. |
| [`docs/tasks/task-0012.md`](file:///playground/tilix/docs/tasks/task-0012.md) | Created | Executable task checklist with all 5 subtasks completed. |
| [`tests/test_phase12_window_geometry.rs`](file:///playground/tilix/tests/test_phase12_window_geometry.rs) | Created | Comprehensive integration test suite verifying font measurement, cell scale, 80x24 calculation, simulated 196 DPI HiDPI calculation, custom grids, chrome toggles, screen clamping, fallbacks, and window instantiation. |
| [`docs/results/result-0012.md`](file:///playground/tilix/docs/results/result-0012.md) | Created | Delivery result record synthesizing Phase 12 completion. |

---

## 3. Validation & Quality Gate Outcomes

### 3.1 Test Suites
- `cargo test --test test_phase12_window_geometry`: **10 passed; 0 failed; 0 ignored**
  - Verified headless VTE font cell measurement returns positive dimensions.
  - Verified 1.5x cell scale factor increases measured dimensions proportionally.
  - Verified default 80x24 profile calculation produces expected window dimensions with default chrome.
  - Verified simulated 196 DPI HiDPI cell (16x35) with 80x24 grid computes ~1296x960 window size instead of the hardcoded 900x600.
  - Verified custom 132x43 terminal grid produces proportionally larger window dimensions (1204x980).
  - Verified chrome toggles accurately deduct pixel offsets when scrollbar, headerbar, tabbar, or pane header are disabled.
  - Verified screen clamping limits oversized grids to 90% of monitor dimensions (720x540 on 800x600 monitor).
  - Verified minimum size clamping prevents windows from falling below 300x200 on tiny grids.
  - Verified headless fallback gracefully returns default (900, 600) when font dimensions are zero or negative.
  - Verified `TilixWindow::new_empty` applies calculated dimensions to the underlying `adw::ApplicationWindow`.
- Full workspace test suite (`cargo test`): **146 unit tests passed; 0 failed; 0 ignored; 100% pass**.

### 3.2 Compiler & Linter Verification
- `cargo check --all-targets`: **0 errors, 0 warnings**
- `cargo test`: Clean exit code 0 across the entire workspace.

---

## 4. Residual Risks & Technical Debt

1. **Wayland Fractional Scaling:**
   - *Observation:* On certain Wayland compositors with fractional scaling, `gdk::Monitor::geometry()` returns logical integer coordinates.
   - *Mitigation:* Clamping to 90% (`MAX_MONITOR_RATIO = 0.90`) provides a generous safety margin preventing windows from exceeding visible screen boundaries under all fractional scaling factors.
2. **Missing Custom Fonts:**
   - *Observation:* If a user specifies a non-existent font name in a profile, Pango falls back to system monospace.
   - *Mitigation:* `vte::Terminal` cell measurement resolves successfully against the fallback font, ensuring valid positive cell dimensions.

---

## 5. Future Milestones

- **Session Window State Persistence:** Save and restore user-resized window geometry on shutdown when a "remember window size" preference is enabled.
- **Dynamic System Font Querying:** Query GSettings `org.gnome.desktop.interface monospace-font-name` dynamically when `use_system_font` is enabled, replacing the default `"Monospace 11"` fallback.
