# Delivery Result Record: Phase 15 — Compact Mode UI Density Optimization

- **Document ID:** `RESULT-0015`
- **Task Reference:** [`docs/tasks/task-0015.md`](file:///playground/tilix/docs/tasks/task-0015.md)
- **Spec Reference:** [`docs/specs/spec-0015-compact-mode-density-2026-09-19.md`](file:///playground/tilix/docs/specs/spec-0015-compact-mode-density-2026-09-19.md)
- **Living Architecture Reference:** [`docs/architecture/overview.md`](file:///playground/tilix/docs/architecture/overview.md) (Section 27)
- **Date:** 2026-09-19
- **Status:** Completed & Verified (Approved by Review Gate)

---

## 1. Executive Summary

Phase 15 introduces **Scheme A: Compact Mode (紧凑模式)** to address screen real estate concerns for tiling terminal users on standard and high-DPI displays:

1. **Dedicated Domain Property (`src/model/config.rs`):**
   - Added `compact_mode: bool` (default: `false`) to `AppConfig`.
   - Guaranteed full backwards-compatible deserialization (`#[serde(default)]`) so existing configurations omitting `compact_mode` deserialize cleanly to `false`.

2. **Window Chrome Density Styling via CSS (`src/ui/window.rs`):**
   - Augmented `setup_css()` with high-specificity `.compact` styling rules across:
     - `toolbarview > .top-bar` & `.collapse-spacing`: padding reset to 0, completely eradicating the 3px Libadwaita spacing gap between the top bars and the terminal content.
     - `headerbar`: min-height compressed to **28px**, zero top/bottom padding; `windowtitle` set to 20px min-height; header buttons set to `22px` with 16px icons.
     - Window control buttons (minimize, maximize, close): bounds reduced to `20px` with `1px` padding, zero vertical margin to prevent clipping while maintaining touch target reliability.
     - `tabbar`, `tabbox`, and child `tab`: `tabbox` compressed to `22px` min-height with 0 padding; child `tab` compressed to `20px` min-height with `0 4px` padding and 18px close buttons; `.box` margin/box-shadow neutralized.
     - `.terminal-pane-header`: height compressed to **22px** with `0 4px` padding, 18px buttons, and `0.85em` label font scaling.

3. **Runtime Weak Reference Tracking & Reactive Projection (`src/ui/window.rs`):**
   - Established thread-local `WINDOW_INSTANCES: RefCell<Vec<glib::WeakRef<adw::ApplicationWindow>>>`.
   - Implemented `register_window_instance` and `apply_compact_mode_to_all_windows(compact: bool)` with automatic dead weak reference pruning.
   - Dynamically broadcasts the `.compact` CSS class across all active windows, header bars, and tab bars with zero application restart required.

4. **Synchronized Geometry Engine (`src/ui/geometry.rs`):**
   - Defined `COMPACT_HEADER_BAR_HEIGHT = 28`, `COMPACT_TAB_BAR_HEIGHT = 22`, and `COMPACT_PANE_HEADER_HEIGHT = 22`.
   - Updated `calculate_window_size_from_cell_size` so window initial size calculation uses compact chrome constants when `app_config.compact_mode` is enabled, recovering an exact **48px** (>2 rows of terminal text) of vertical workspace.
   - Preserves dimension invariance when chrome is toggled off (`WindowStyle::HideToolbar`, `show_tab_bar = false`, `PaneTitleStyle::None`).

5. **Preferences UI Integration (`src/ui/preferences.rs`):**
   - Added an interactive `adw::SwitchRow` for "Compact Mode" under Preferences -> Appearance -> Window group.
   - Synchronized switch state with `save_app` and connected `connect_active_notify` for instant live updates.

6. **Living Architecture & Automated Test Verification:**
   - Updated `docs/architecture/overview.md` to version `0.15.0` with Section 27 documenting the Compact Mode density architecture.
   - Authored new integration test suite `tests/test_phase15_compact_mode.rs` covering all 9 test scenarios specified in `SPEC-0015`.
   - Full workspace test suite passes with **185 passed, 0 failed** and zero regressions.

---

## 2. Completed Work & Delivered Files

| File | Status | Description |
| :--- | :--- | :--- |
| [`src/model/config.rs`](file:///playground/tilix/src/model/config.rs) | Modified | Added `#[serde(default)] pub compact_mode: bool`, default `false`, and serde roundtrip/compatibility tests. |
| [`src/ui/geometry.rs`](file:///playground/tilix/src/ui/geometry.rs) | Modified | Defined `COMPACT_*` chrome height constants and integrated compact mode into `calculate_window_size_from_cell_size`. |
| [`src/ui/window.rs`](file:///playground/tilix/src/ui/window.rs) | Modified | Added `.compact` CSS rules, `WINDOW_INSTANCES` weak registry, `register_window_instance`, `apply_compact_mode_to_all_windows`, and window initialization logic. |
| [`src/ui/preferences.rs`](file:///playground/tilix/src/ui/preferences.rs) | Modified | Added Compact Mode SwitchRow to Appearance -> Window group, wired to `save_app` and live broadcast. |
| [`docs/architecture/overview.md`](file:///playground/tilix/docs/architecture/overview.md) | Modified | Bumped version to 0.15.0 and added Section 27 documenting Compact Mode density architecture. |
| [`docs/specs/spec-0015-compact-mode-density-2026-09-19.md`](file:///playground/tilix/docs/specs/spec-0015-compact-mode-density-2026-09-19.md) | Created | Master architectural specification for Phase 15. |
| [`docs/tasks/task-0015.md`](file:///playground/tilix/docs/tasks/task-0015.md) | Created | Executable task checklist with all subtasks verified and marked complete. |
| [`tests/test_phase15_compact_mode.rs`](file:///playground/tilix/tests/test_phase15_compact_mode.rs) | Created | Automated headless integration test suite covering 9 test cases. |
| [`docs/results/result-0015.md`](file:///playground/tilix/docs/results/result-0015.md) | Created | Delivery result record synthesized by the orchestrator. |

---

## 3. Validation Outcomes & Test Results

### 3.1 Targeted Phase 15 Integration Tests
Command: `cargo test --test test_phase15_compact_mode`
```text
running 9 tests
test test_phase15_compact_mode_default_is_false ... ok
test test_phase15_config_serde_roundtrip ... ok
test test_phase15_backwards_compatibility_deserialization ... ok
test test_phase15_geometry_constants_integrity ... ok
test test_phase15_window_geometry_compact_vs_normal ... ok
test test_phase15_window_geometry_compact_with_hidden_chrome ... ok
test test_phase15_setup_css_loads_compact_rules ... ok
test test_phase15_reactive_projection_window_css_class ... ok
test test_phase15_tilix_window_compact_mode_initialization ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
```

### 3.2 In-Tree Unit Tests
- `cargo test model::config`: 16 passed, 0 failed.
- `cargo test ui::geometry`: 3 passed, 0 failed.
- `cargo test ui::window`: 9 passed, 0 failed.

### 3.3 Full Workspace Regression Suite
Command: `cargo test`
```text
test result: ok. 185 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
```

---

## 4. Residual Risks & Tech Debt

1. **Window Control Button Sizing Across Desktop Environments:**
   - Under custom desktop themes or atypical GTK window manager decorations, window control buttons might vary in baseline padding. Standard 24px button sizing with 2px padding prevents icon clipping on default Libadwaita / Adwaita themes.
2. **Fixed Presets vs Arbitrary Granularity:**
   - Compact Mode uses fixed, curated geometry offsets (34px headerbar, 28px tabbar, 26px pane header) rather than user-configurable pixel sliders. This ensures layout stability, zero text clipping, and predictable geometry calculations. If multi-level density (e.g. Scheme B `UiDensity`) is required in the future, the underlying architecture can be naturally extended.

---

## 5. Future Milestones

- **Phase 16 (Optional):** Scheme B expansion (`UiDensity: Default / Compact / Comfortable`) if tactile touch display optimization is requested.
- **Packaging & Release:** Merge Phase 15 deliverables into main release tags.
