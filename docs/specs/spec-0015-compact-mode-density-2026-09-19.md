# Master Specification: Phase 15 — Compact Mode UI Density Optimization

- **Document ID:** `SPEC-0015`
- **Date:** 2026-09-19
- **Status:** Proposed / Ready for Execution
- **Workflow Level:** Medium/Large (Phase 15 of Tilix Rewrite)
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
  - [`docs/specs/spec-0012-dynamic-window-geometry-default-size-2026-09-18.md`](file:///playground/tilix/docs/specs/spec-0012-dynamic-window-geometry-default-size-2026-09-18.md)
  - [`docs/specs/spec-0013-terminal-zoom-and-alt-drag-2026-09-18.md`](file:///playground/tilix/docs/specs/spec-0013-terminal-zoom-and-alt-drag-2026-09-18.md)
  - [`docs/specs/spec-0014-osc52-and-clipboard-integration-2026-09-19.md`](file:///playground/tilix/docs/specs/spec-0014-osc52-and-clipboard-integration-2026-09-19.md)
- **Target Location:** `/playground/tilix`

---

## 1. Context & Motivation

Terminal power users frequently work on laptops, ultra-wide multi-column layouts, or compact display setups where vertical display real estate is precious. Every vertical pixel consumed by window chrome (decorations, header bars, tab bars, pane title bars) reduces the number of terminal command output rows visible without scrolling.

In standard GNOME Libadwaita / GTK4 styling, interface controls prioritize generous touch targets and relaxed padding:
- Main Window `adw::HeaderBar`: default minimum height is **46px** (with 34px buttons and padding).
- Tab Bar `adw::TabBar`: default minimum height is **38px** (with generous tab button padding).
- Split Pane Title Bar (`terminal-pane-header`): default height is **36px**.

Combined, standard window chrome occupies **120px** of vertical height before accounting for window borders. On a standard 1080p display with a 10pt monospace font (~20px cell height), 120px of chrome consumes **6 entire rows** of terminal workspace.

While Phase 5 (`spec-0005`) introduced `WindowStyle::HideToolbar` to completely remove the headerbar, many users still want window controls, active tab indicators, and split buttons accessible, but with **high-density UI presentation**.

Phase 15 introduces **Scheme A: Compact Mode (紧凑模式)**:
1. **Dedicated Setting:** `compact_mode: bool` (default: `false`) in `AppConfig` (`src/model/config.rs`).
2. **Backwards Compatibility:** Full serde backward compatibility (`#[serde(default)]`) ensuring legacy configurations load smoothly without schema corruption.
3. **Preferences UI Integration:** An interactive switch row (`adw::SwitchRow`) in `Preferences -> Appearance -> Window` labeled "Compact Mode".
4. **Reactive Projection:** Instantaneous runtime broadcast (`apply_compact_mode_to_all_windows(compact: bool)`) toggling the `.compact` CSS class across all active windows without requiring application restart.
5. **CSS Density Styling:**
   - HeaderBar min-height compressed from 46px to **34px**, with compact 24px buttons, 16px icons, and window controls padding adjusted to prevent clipping.
   - TabBar min-height compressed from 38px to **28px** with compact tab padding.
   - TerminalPane header compressed from 36px to **26px** with reduced padding and proportional font sizing.
6. **Dynamic Geometry Synchronization:** In `src/ui/geometry.rs`, compact chrome heights (`COMPACT_HEADER_BAR_HEIGHT = 34`, `COMPACT_TAB_BAR_HEIGHT = 28`, `COMPACT_PANE_HEADER_HEIGHT = 26`) are integrated into `calculate_window_size_from_cell_size`, ensuring that initial window dimensions for default row/column configurations match the active density mode precisely.

---

## 2. Amended Documents & Scope

- **Amended Document:** [`docs/architecture/overview.md`](file:///playground/tilix/docs/architecture/overview.md)
- **Status of Living Architecture:** Active (Version bumped to 0.15.0 for Phase 15)

### 2.1 Affected Scope

1. **`src/model/config.rs`:**
   - Add `pub compact_mode: bool` to `AppConfig` struct with default `false`.
   - Ensure serde deserialization defaults to `false` for older configuration payloads missing this key.
   - Add unit tests verifying default value, JSON serialization, and legacy backwards compatibility.
2. **`src/ui/geometry.rs`:**
   - Define public constants:
     * `pub const COMPACT_HEADER_BAR_HEIGHT: i32 = 34;`
     * `pub const COMPACT_TAB_BAR_HEIGHT: i32 = 28;`
     * `pub const COMPACT_PANE_HEADER_HEIGHT: i32 = 26;`
   - Update `calculate_window_size_from_cell_size` to use compact chrome heights when `app_config.compact_mode` is `true`.
3. **`src/ui/window.rs`:**
   - Extend `setup_css()` with `.compact` styling rules for `headerbar`, `button`, `windowcontrols`, `tabbar`, and `.terminal-pane-header`.
   - Track window instances via `WINDOW_INSTANCES: RefCell<Vec<glib::WeakRef<adw::ApplicationWindow>>>`.
   - Apply `.compact` CSS class to window, header bar, and tab bar during window creation when `compact_mode` is enabled.
   - Implement `pub fn apply_compact_mode_to_all_windows(compact: bool)` to dynamically add/remove the `.compact` CSS class from all active windows, header bars, and tab bars.
   - Expose helper `pub fn register_window_instance(window: &adw::ApplicationWindow)` to allow test fixtures to verify reactive projection cleanly.
4. **`src/ui/preferences.rs`:**
   - Add `compact_mode_row: adw::SwitchRow` to the "Window" preferences group under the Appearance page.
   - In `save_app` callback, synchronize `cfg.compact_mode` from the switch state, persist to disk, and trigger `apply_compact_mode_to_all_windows(cfg.compact_mode)`.
   - Connect `compact_mode_row.connect_active_notify` to trigger `save_app`.
5. **`tests/test_phase15_compact_mode.rs` (New Integration Suite):**
   - Headless verification of default config, serde roundtrip, backwards compatibility, geometry calculations in normal vs compact mode, CSS parsing, and reactive window projection.

### 2.2 Unaffected Scope

- `src/pty/*`: Shell spawning, OSC 52, and PTY streams remain unchanged.
- `src/model/layout.rs`: Split tree topology, docking math, and layout balancing remain unchanged.
- `src/model/keybindings.rs`: Shortcut bindings and action catalogs remain unchanged.
- `src/model/profile.rs`: Profile font, colors, and terminal properties remain unchanged.
- `src/model/title.rs`: Dynamic title token formatting remains unchanged.
- `src/ui/dnd.rs`: Pane drag-and-drop mechanics remain unchanged.

---

## 3. Goals & Non-Goals

### Goals

1. **Persistent Configuration Schema:**
   - Provide a clean, boolean setting `compact_mode` in `AppConfig`.
   - Guarantee 100% serde backwards compatibility: existing `config.json` files without `compact_mode` must deserialize seamlessly without errors, defaulting `compact_mode` to `false`.
2. **Instant Reactive UI Projection:**
   - Toggling the switch in Preferences must immediately update all open windows, headerbars, tab bars, and pane headers without restarting Tilix.
3. **Pixel-Perfect Compact CSS Rules:**
   - HeaderBar min-height: 34px (down from standard 46px).
   - HeaderBar buttons: 24px min-height / min-width, 16px icon size.
   - Window controls buttons (minimize, maximize, close): adjusted padding without clipping icons or touch targets.
   - TabBar min-height: 28px (down from standard 38px).
   - TerminalPane header: 26px min-height (down from standard 36px) with tight padding (`1px 4px`) and 0.85em font scaling.
4. **Coordinated Window Geometry Engine:**
   - `calculate_window_size_from_cell_size` in `src/ui/geometry.rs` must dynamically calculate window dimensions using compact chrome constants (`34px`, `28px`, `26px`) when `compact_mode` is enabled, resulting in an exact 32px height savings on default 80x24 configurations.
5. **Preferences UI Parity:**
   - Place "Compact Mode" prominently in Preferences -> Appearance under the "Window" group with informative subtitle copy explaining its function.
6. **Automated Headless Test Suite:**
   - Comprehensive headless tests covering domain models, serde, geometry calculations, CSS loading, and GTK reactive projection.

### Non-Goals

- Dynamically resizing existing on-screen windows when toggling compact mode (in GNOME HIG and upstream Tilix, window dimensions are controlled by the window manager or user; toggling density compresses internal chrome to yield more terminal canvas space within the existing window bounds).
- Arbitrary custom pixel padding sliders (fixed compact presets preserve visual polish and prevent layout breakage).
- Altering the terminal font size or cell line spacing via compact mode (terminal grid font size is strictly governed by `Profile.font` and cell scaling properties).

---

## 4. Architecture & Design Decisions

### 4.1 Decision Gating: Facts, Decisions, Assumptions, Deferred Items

- **Facts:**
  - GTK4 / Libadwaita styling is controlled hierarchically via CSS classes attached to widgets (`window.add_css_class("compact")`).
  - CSS providers loaded into `gtk::style_context_add_provider_for_display` with priority `STYLE_PROVIDER_PRIORITY_APPLICATION` apply globally to all windows.
  - `src/ui/geometry.rs` provides pure-domain window sizing functions used when spawning new windows.
  - Window header bars and tab bars are already tracked in `src/ui/window.rs` via thread-local weak reference vectors (`WINDOW_HEADER_BARS`, `WINDOW_TAB_BARS`).
- **Decisions:**
  - **Scheme A (Dedicated Boolean Setting):** Adopt a clean boolean toggle `compact_mode: bool` on `AppConfig` rather than multi-level density enums or numeric padding overrides. This gives an unambiguous, high-contrast user experience.
  - **CSS Class Targeting:** Apply the class `.compact` at the window root (`window.compact`) as well as on top bars (`headerbar.compact`, `tabbar.compact`). This allows both descendant selectors (`window.compact headerbar`) and direct widget selectors (`headerbar.compact`) to match consistently.
  - **Weak Window Registry:** Introduce `static WINDOW_INSTANCES: RefCell<Vec<glib::WeakRef<adw::ApplicationWindow>>>` in `src/ui/window.rs` with automatic dead-reference pruning during broadcasts, matching the existing pattern used for `WINDOW_HEADER_BARS`.
  - **Geometry Sizing Constants:**
    * `DEFAULT_HEADER_BAR_HEIGHT = 46` vs `COMPACT_HEADER_BAR_HEIGHT = 34` (Δ = 12px)
    * `DEFAULT_TAB_BAR_HEIGHT = 38` vs `COMPACT_TAB_BAR_HEIGHT = 28` (Δ = 10px)
    * `DEFAULT_PANE_HEADER_HEIGHT = 36` vs `COMPACT_PANE_HEADER_HEIGHT = 26` (Δ = 10px)
    * Total chrome height savings when all chrome is visible: **32px** (approx 1.5 to 2 terminal lines).
- **Assumptions:**
  - Libadwaita windowcontrols buttons will render cleanly at 24px height with 2px padding on standard and high-DPI displays without icon clipping.
- **Deferred Items:**
  - Per-profile compact density settings (compact mode is an application-level appearance choice matching window style and tab bar visibility).

### 4.2 Architecture Decision Table

| Option | Pros | Cons | Verdict |
| :--- | :--- | :--- | :--- |
| **Option 1: Scheme A — Dedicated Compact Mode Toggle (Chosen)** | Simple binary switch; predictable layout; matches user request; instant reactive CSS switching; exact geometry math. | Single density preset rather than infinite gradations. | **Accepted** |
| **Option 2: Fine-Grained Sliders (Padding / Min-Heights)** | Allows custom pixel-level tuning for edge cases. | Overcomplicates Preferences UI; high risk of widget clipping and text overflow; difficult geometry calculation. | **Rejected** |
| **Option 3: Global Zoom / Scale Factor** | Scales entire UI including fonts. | Blurs vector icons; distorts font rendering; conflicts with font size preferences in profiles. | **Rejected** |

---

## 5. Detailed Technical Specifications

### 5.1 Domain Model & Configuration Schema (`src/model/config.rs`)

`AppConfig` is extended with `compact_mode: bool`:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub default_profile: Profile,
    pub profiles: Vec<Profile>,
    pub default_profile_id: String,
    pub quake_height_percent: u32,
    pub quake_hide_on_unfocus: bool,
    pub notifications_enabled: bool,
    pub bell_notifications: bool,
    pub process_exit_notifications: bool,
    pub window_style: WindowStyle,
    pub use_wide_handle: bool,
    pub pane_title_style: PaneTitleStyle,
    pub pane_title_show_when_single: bool,
    pub show_tab_bar: bool,
    pub keybindings: KeybindingsConfig,
    #[serde(default = "default_session_name_default")]
    pub default_session_name: String,
    #[serde(default = "default_app_title_default")]
    pub app_title: String,
    #[serde(default)]
    pub compact_mode: bool,
}
```

In `impl Default for AppConfig`:
```rust
compact_mode: false,
```

#### Serialization Backwards Compatibility Guarantee:
Legacy JSON strings lacking `"compact_mode"` will deserialize through `#[serde(default)]` and receive `false`. When serialized, `compact_mode: true` or `false` is persisted as a standard JSON boolean.

---

### 5.2 Window Chrome Geometry Engine (`src/ui/geometry.rs`)

`src/ui/geometry.rs` defines the explicit compact chrome height constants:

```rust
pub const COMPACT_HEADER_BAR_HEIGHT: i32 = 28;
pub const COMPACT_TAB_BAR_HEIGHT: i32 = 22;
pub const COMPACT_PANE_HEADER_HEIGHT: i32 = 22;
```

In `calculate_window_size_from_cell_size`:
```rust
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
```

---

### 5.3 CSS Theme & Density Styling (`src/ui/window.rs`)

In `setup_css()`, the application stylesheet is augmented with the following rules:

```css
/* =========================================================================
 * Compact Mode Density Overrides
 * ========================================================================= */
window.compact headerbar,
.compact headerbar,
headerbar.compact {
    min-height: 34px;
    padding-top: 0;
    padding-bottom: 0;
}

window.compact headerbar button,
.compact headerbar button {
    min-height: 24px;
    min-width: 24px;
    padding: 2px 4px;
}

window.compact headerbar button image,
.compact headerbar button image {
    -gtk-icon-size: 16px;
}

window.compact headerbar windowcontrols button,
.compact headerbar windowcontrols button {
    min-height: 24px;
    min-width: 24px;
    padding: 2px;
    margin-top: 0;
    margin-bottom: 0;
}

window.compact tabbar,
.compact tabbar,
tabbar.compact {
    min-height: 28px;
}

window.compact tabbar tab,
.compact tabbar tab {
    min-height: 28px;
    padding: 2px 6px;
}

window.compact .terminal-pane-header,
.compact .terminal-pane-header,
.terminal-pane-header.compact {
    padding: 1px 4px;
    min-height: 26px;
}

window.compact .terminal-pane-header button,
.compact .terminal-pane-header button {
    min-height: 20px;
    min-width: 20px;
    padding: 1px 2px;
}

window.compact .terminal-pane-header label,
.compact .terminal-pane-header label {
    font-size: 0.85em;
}
```

---

### 5.4 Reactive Projection & Window Lifecycle (`src/ui/window.rs`)

1. **Thread-Local Window Instance Tracking:**
```rust
thread_local! {
    ...
    static WINDOW_INSTANCES: RefCell<Vec<glib::WeakRef<adw::ApplicationWindow>>> = const { RefCell::new(Vec::new()) };
}
```

2. **Window Creation Hook (`TilixWindow::new_empty_with_profile`):**
```rust
if cfg.compact_mode {
    window.add_css_class("compact");
    header_bar.add_css_class("compact");
    tab_bar.add_css_class("compact");
}
WINDOW_INSTANCES.with(|wins| wins.borrow_mut().push(window.downgrade()));
```

3. **Public Registration Function (for Tests & Lifecycle):**
```rust
pub fn register_window_instance(window: &adw::ApplicationWindow) {
    WINDOW_INSTANCES.with(|wins| wins.borrow_mut().push(window.downgrade()));
}
```

4. **Reactive Broadcast Implementation:**
```rust
pub fn apply_compact_mode_to_all_windows(compact: bool) {
    WINDOW_INSTANCES.with(|wins| {
        wins.borrow_mut().retain(|win_weak| {
            if let Some(win) = win_weak.upgrade() {
                if compact {
                    win.add_css_class("compact");
                } else {
                    win.remove_css_class("compact");
                }
                true
            } else {
                false
            }
        });
    });

    WINDOW_HEADER_BARS.with(|bars| {
        bars.borrow_mut().retain(|bar_weak| {
            if let Some(bar) = bar_weak.upgrade() {
                if compact {
                    bar.add_css_class("compact");
                } else {
                    bar.remove_css_class("compact");
                }
                true
            } else {
                false
            }
        });
    });

    WINDOW_TAB_BARS.with(|bars| {
        bars.borrow_mut().retain(|bar_weak| {
            if let Some(bar) = bar_weak.upgrade() {
                if compact {
                    bar.add_css_class("compact");
                } else {
                    bar.remove_css_class("compact");
                }
                true
            } else {
                false
            }
        });
    });
}
```

---

### 5.5 Preferences Dialog UI (`src/ui/preferences.rs`)

In `TilixPreferencesWindow::new`:
Under `// APPEARANCE PAGE (Window & Title)` -> `window_group`:

```rust
let compact_mode_row = adw::SwitchRow::new();
compact_mode_row.set_title("Compact Mode");
compact_mode_row.set_subtitle("Reduce padding and titlebar heights for maximum terminal display area");
compact_mode_row.set_active(current_config.borrow().compact_mode);
window_group.add(&compact_mode_row);
```

In `save_app` synchronization closure:
```rust
let c_mode = compact_mode_row.clone();
...
let compact_mode = c_mode.is_active();
...
cfg.compact_mode = compact_mode;
...
let _ = cfg.save();
crate::ui::window::apply_compact_mode_to_all_windows(cfg.compact_mode);
```

Connect notification signal:
```rust
let s6 = Rc::clone(&save_app);
compact_mode_row.connect_active_notify(move |_| s6());
```

---

## 6. File Manifest & Detailed Code Modifications

| File | Modification Type | Description of Changes |
| :--- | :--- | :--- |
| `src/model/config.rs` | Modified | Add `compact_mode: bool` to `AppConfig`, implement default, add unit tests for defaults and backwards compatibility. |
| `src/ui/geometry.rs` | Modified | Add constants `COMPACT_HEADER_BAR_HEIGHT`, `COMPACT_TAB_BAR_HEIGHT`, `COMPACT_PANE_HEADER_HEIGHT`; update `calculate_window_size_from_cell_size`. |
| `src/ui/window.rs` | Modified | Add compact CSS rules to `setup_css()`, track `WINDOW_INSTANCES`, add `.compact` class on window creation, implement `apply_compact_mode_to_all_windows`. |
| `src/ui/preferences.rs` | Modified | Add `compact_mode_row` to Window preferences group, wire into `save_app` and connect active notification. |
| `docs/architecture/overview.md` | Modified | Update architecture version to 0.15.0 and document Compact Mode UI density optimization. |
| `docs/specs/spec-0015-compact-mode-density-2026-09-19.md` | Created | Master architectural specification for Phase 15. |
| `docs/tasks/task-0015.md` | Created | Executable step-by-step task checklist. |
| `tests/test_phase15_compact_mode.rs` | Created | Automated headless integration test suite for Phase 15. |

---

## 7. Verification & Automated Test Strategy

### 7.1 Headless Unit & Integration Tests

The test suite `tests/test_phase15_compact_mode.rs` covers:
1. **`test_phase15_compact_mode_default_is_false`:**
   - Asserts `AppConfig::default().compact_mode == false`.
2. **`test_phase15_config_serde_roundtrip`:**
   - Serializes `AppConfig` with `compact_mode = true` and `compact_mode = false`.
   - Deserializes and verifies exact match.
3. **`test_phase15_backwards_compatibility_deserialization`:**
   - Deserializes a legacy JSON string without `compact_mode`.
   - Verifies deserialization succeeds and sets `compact_mode = false`.
4. **`test_phase15_geometry_constants_integrity`:**
   - Asserts `COMPACT_HEADER_BAR_HEIGHT == 34`, `COMPACT_TAB_BAR_HEIGHT == 28`, `COMPACT_PANE_HEADER_HEIGHT == 26`.
5. **`test_phase15_window_geometry_compact_vs_normal`:**
   - Using fixed cell size `(9, 20)` and 80x24 profile:
     * Normal height: `24 * 20 + 36 + 46 + 38 = 600px`
     * Compact height: `24 * 20 + 26 + 34 + 28 = 568px`
     * Total vertical delta: exactly 32px saved.
     * Width: unchanged (736px).
6. **`test_phase15_window_geometry_compact_with_hidden_chrome`:**
   - Tests `WindowStyle::HideToolbar`, `show_tab_bar = false`, `PaneTitleStyle::None`.
   - Asserts both normal and compact modes evaluate to identical terminal-only dimensions `(736, 480)`.
7. **`test_phase15_setup_css_loads_compact_rules`:**
   - Wrapped in `run_gtk_test`, calls `setup_css()` and verifies CSS parsing executes without errors.
8. **`test_phase15_reactive_projection_window_css_class`:**
   - Wrapped in `run_gtk_test`, instantiates an `adw::ApplicationWindow`.
   - Registers with `register_window_instance(&win)`.
   - Calls `apply_compact_mode_to_all_windows(true)` -> verifies `win.has_css_class("compact")`.
   - Calls `apply_compact_mode_to_all_windows(false)` -> verifies `!win.has_css_class("compact")`.
9. **`test_phase15_tilix_window_compact_mode_initialization`:**
   - Wrapped in `run_gtk_test`, creates `TilixWindow` with `compact_mode = true` config and verifies window has `"compact"` CSS class.

### 7.2 Full Regression Suite
- Run `cargo test` across all crates and integration test suites (`test_phase1` through `test_phase15`).

---

## 8. Residual Risks, Safety Guarantees & Tech Debt

| Risk | Impact | Mitigation Strategy |
| :--- | :--- | :--- |
| **Window Controls Button Clipping** | Window close/minimize buttons might look cramped or clip window border. | Reduced padding with `margin: 0; min-height: 24px; min-width: 24px;` tested across Libadwaita themes; icons remain standard 16px. |
| **Old Configuration Parse Failure** | Users upgrading from Phase 14 might lose preferences if deserialization fails. | Handled via `#[serde(default)]` on `AppConfig` and `compact_mode` field; verified with explicit backwards-compatibility test. |
| **Memory Leaks in Window Tracking** | Retaining dead window references in `WINDOW_INSTANCES` could leak memory. | Window references are stored as `glib::WeakRef`; dead references are automatically pruned during every broadcast call via `retain(...)`. |
| **Dynamic Resizing of Active Windows** | User expects existing window to shrink when compact mode is toggled. | Explicitly documented as Non-Goal: window outer bounds remain steady while interior terminal canvas expands by 32px. |

---

## 9. Rollout & Milestone Plan

- **Phase 15.1:** Model extension and backwards compatibility serialization tests.
- **Phase 15.2:** Geometry engine constants and compact dimension calculation.
- **Phase 15.3:** CSS stylesheet rules and reactive broadcast projection in `src/ui/window.rs`.
- **Phase 15.4:** Preferences UI `adw::SwitchRow` integration.
- **Phase 15.5:** Integration test suite `tests/test_phase15_compact_mode.rs` and architecture overview documentation.
- **Phase 15.6:** Full workspace test execution and review gate delivery.
