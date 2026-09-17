# Master Specification: Phase 6 — Terminal Pane Title & Toolbar Visibility Controls

- **Document ID:** `SPEC-0006`
- **Date:** 2026-09-17
- **Status:** Approved / Decision Ready
- **Workflow Level:** Medium/Large (Phase 6 of Tilix Rewrite)
- **Preceding Specifications:**
  - [`docs/specs/spec-0004-packaging-distribution-polish-2026-09-16.md`](file:///playground/tilix/docs/specs/spec-0004-packaging-distribution-polish-2026-09-16.md)
  - [`docs/specs/spec-0005-window-style-wide-handle-2026-09-17.md`](file:///playground/tilix/docs/specs/spec-0005-window-style-wide-handle-2026-09-17.md)
- **Target Location:** `/playground/tilix`

---

## 1. Context & Motivation

In the original Tilix terminal emulator, the user interface features two distinct hierarchical levels of toolbars and titlebars:
1. **Window-Level Header Bar (`adw::HeaderBar`):** The top-level window title and global control bar (housing window close buttons, new tab, global sync toggle, and preferences). Phase 5 introduced `WindowStyle` (`Normal` vs `HideToolbar`) to toggle the visibility of this top-level header bar.
2. **Pane-Level Header Bar (`TerminalPane` Header):** Each individual terminal split pane embeds its own dedicated header bar (`terminal-pane-header`). This bar displays the pane title, individual input synchronization toggle button, split horizontal/vertical buttons, and pane close button.

While power users often desire a completely distraction-free, minimalist terminal window, they differentiate between hiding the top-level window chrome and hiding the per-pane headers. Specifically:
- When only a single terminal pane is open in a tab, displaying a pane header is redundant with the window/tab title bar, wasting vertical terminal space.
- When multiple terminal splits exist, pane headers provide crucial visual boundaries, pane titles, and mouse-clickable split/close actions.
- Some power users prefer hiding pane headers entirely (`terminal-title-style: none`) in favor of pure keyboard accelerators (`Ctrl+Shift+R`, `Ctrl+Shift+D`, `Ctrl+Shift+W`, `Ctrl+Shift+I`).

Original Tilix governed this behavior using two key settings:
- `terminal-title-style`: `normal` (display header) vs `none` (completely hidden).
- `terminal-title-show-when-single`: `boolean` (whether to show the pane header when only one pane exists in the session).

Following user clarification (Direction C), Phase 6 introduces first-class support for both window style customization (from Phase 5) AND terminal pane title / toolbar visibility controls.

---

## 2. Amended Documents & Scope

- **Amended Document:** [`docs/specs/spec-0005-window-style-wide-handle-2026-09-17.md`](file:///playground/tilix/docs/specs/spec-0005-window-style-wide-handle-2026-09-17.md)
- **Status of SPEC-0005:** Active (Partially Amended)

### 2.1 Affected Scope in SPEC-0005
- **Section 1 (Context & Motivation) & Section 3 (Scope Boundaries):**
  - Clarified that Phase 5's `WindowStyle::HideToolbar` specifically governs the window-level `adw::HeaderBar` within `TilixWindow`, while Phase 6 establishes independent domain and projection controls over `TerminalPane` header bars.
  - The phrase "Hide Toolbar" in Phase 5 is strictly preserved for top-level window chrome, preventing regressions or breaking changes to `WindowStyle`.

### 2.2 Unaffected Scope in SPEC-0005
- `WindowStyle` enum (`Normal`, `HideToolbar`) and its serialization format in `AppConfig`.
- `use_wide_handle: bool` configuration toggle and `SessionView` paned projection.
- `WINDOW_HEADER_BARS` weak-reference registry in `src/ui/window.rs`.
- All Phase 5 unit and integration tests (`tests/test_phase5_window_style_wide_handle.rs`).

### 2.3 Rationale & Migration Strategy
- Provides clear architectural separation between window-level chrome (`adw::HeaderBar`) and session-level pane chrome (`TerminalPane` header).
- Pure additive schema migration in `AppConfig`: new fields `pane_title_style: PaneTitleStyle` and `pane_title_show_when_single: bool` default via `#[serde(default)]` to preserve existing user configs seamlessly.

---

## 3. Goals & Non-Goals

### Goals
1. **Domain Model Extension:**
   - Define `PaneTitleStyle` enum (`Normal`, `None`) with `#[serde(rename_all = "snake_case")]` in `src/model/config.rs`.
   - Extend `AppConfig` with `pub pane_title_style: PaneTitleStyle` (default `Normal`) and `pub pane_title_show_when_single: bool` (default `true`).
   - Maintain 100% backwards and forwards JSON compatibility.
2. **`TerminalPane` Header Visibility API:**
   - Provide `set_header_visible(&self, visible: bool)` and `is_header_visible(&self) -> bool` on `TerminalPane` in `src/ui/terminal_pane.rs`.
3. **`SessionView` Reactive Projection:**
   - Store active `pane_title_style` and `pane_title_show_when_single` settings in `SessionView`.
   - Dynamically compute header visibility based on current pane count:
     - If `PaneTitleStyle::None`: always `false`.
     - If `PaneTitleStyle::Normal`: if `pane_count <= 1`, evaluate `pane_title_show_when_single`; if `pane_count > 1`, evaluate `true`.
   - Re-evaluate visibility automatically upon splits (`split_pane_with_dir`), closures (`close_pane`), layout resets (`reset`), and setting changes without restarting terminal processes or losing focus.
4. **Global Window-Layer Broadcast:**
   - Implement `apply_pane_title_settings_to_all_sessions(style, show_when_single)` in `src/ui/window.rs` using the `WIDGET_TO_SESSION` registry.
5. **Preferences UI Controls:**
   - In `src/ui/preferences.rs`, add a dedicated "Terminal Title" group in the "Appearance" page with a `ComboRow` for "Title Style" and a `SwitchRow` for "Show title when single terminal".
   - Wire dynamic signal notifications to trigger persistence and live broadcast.
6. **Headless Testing & Quality Standards:**
   - 100% headless CI testability for domain models and JSON compatibility.
   - Zero compilation warnings under `cargo check --all-targets` and `cargo clippy --all-targets -- -D warnings`.

### Non-Goals
- Modifying terminal keyboard shortcuts (`Ctrl+Shift+R`, `Ctrl+Shift+D`, `Ctrl+Shift+W`, `Ctrl+Shift+I` remain fully functional when headers are hidden).
- Per-pane or per-profile overriding of title styles (kept global in `AppConfig`, consistent with Tilix architecture).

---

## 4. Decision Gating Log

| Topic | Resolution | Source | Reason |
| :--- | :--- | :--- | :--- |
| **Title Style Options** | `PaneTitleStyle::Normal` and `PaneTitleStyle::None` | Decision | Matches original Tilix `terminal-title-style` (`normal`, `none`). |
| **Title Style Serialization** | `#[serde(rename_all = "snake_case")]` (`"normal"`, `"none"`) | Decision | Standard Rust/JSON naming convention across `AppConfig`. |
| **Default Settings** | `pane_title_style: PaneTitleStyle::Normal`, `pane_title_show_when_single: true` | Decision | Matches original Tilix default and maintains visual continuity for existing installations. |
| **Visibility Evaluation Rule** | `fn should_show_header(style, show_single, count)`: `None` $\rightarrow$ `false`; `Normal` $\rightarrow$ if `count <= 1` then `show_single` else `true` | Fact | Accurately models Tilix behavior where multi-pane splits display headers while single pane honors user preference. |
| **In-Place Widget Updates** | Toggle `header.set_visible(visible)` on existing `TerminalPane` instances | Decision | Prevents terminal destruction, PTY process reset, or scrollback loss; transitions are flicker-free. |
| **Lifecycle Integration** | Hook visibility evaluation into `rebuild_projection_with` and `set_pane_title_settings` | Decision | Ensures pane headers are automatically synchronized when splits are added, panes closed, or preferences updated. |
| **Accelerator Retention** | Keyboard shortcuts registered on `AdwApplicationWindow` | Fact | Hiding the pane header hides visual buttons but leaves all terminal accelerators (`Ctrl+Shift+*`) active. |

---

## 5. Functional Requirements

### FR-1: Domain Model Extension (`src/model/config.rs` & `src/model/mod.rs`)
1. Define `PaneTitleStyle`:
   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
   #[serde(rename_all = "snake_case")]
   pub enum PaneTitleStyle {
       #[default]
       Normal,
       None,
   }
   ```
2. Extend `AppConfig`:
   ```rust
   #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
   #[serde(default)]
   pub struct AppConfig {
       pub default_profile: Profile,
       pub quake_height_percent: u32,
       pub quake_hide_on_unfocus: bool,
       pub notifications_enabled: bool,
       pub bell_notifications: bool,
       pub process_exit_notifications: bool,
       pub window_style: WindowStyle,
       pub use_wide_handle: bool,
       pub pane_title_style: PaneTitleStyle,
       pub pane_title_show_when_single: bool,
   }
   ```
3. Update `Default for AppConfig`:
   - `pane_title_style: PaneTitleStyle::Normal`
   - `pane_title_show_when_single: true`
4. Re-export `PaneTitleStyle` in `src/model/mod.rs`.

### FR-2: `TerminalPane` Header Visibility (`src/ui/terminal_pane.rs`)
1. Implement public helper methods on `TerminalPane`:
   ```rust
   pub fn set_header_visible(&self, visible: bool) {
       self.header.set_visible(visible);
   }

   pub fn is_header_visible(&self) -> bool {
       self.header.get_visible()
   }
   ```

### FR-3: `SessionView` Reactive Projection (`src/ui/session_view.rs`)
1. Store settings in `SessionView`:
   ```rust
   pub struct SessionView {
       ...
       pane_title_style: Rc<RefCell<PaneTitleStyle>>,
       pane_title_show_when_single: Rc<RefCell<bool>>,
   }
   ```
2. Initialize in `with_model_and_dir`:
   ```rust
   let cfg = crate::model::AppConfig::load();
   let pane_title_style = Rc::new(RefCell::new(cfg.pane_title_style));
   let pane_title_show_when_single = Rc::new(RefCell::new(cfg.pane_title_show_when_single));
   ```
3. Implement `update_pane_headers_visibility`:
   ```rust
   pub fn update_pane_headers_visibility(&self) {
       let count = self.pane_count();
       let style = *self.pane_title_style.borrow();
       let show_single = *self.pane_title_show_when_single.borrow();
       let visible = match style {
           PaneTitleStyle::None => false,
           PaneTitleStyle::Normal => {
               if count <= 1 {
                   show_single
               } else {
                   true
               }
           }
       };
       for pane in self.panes.borrow().values() {
           pane.set_header_visible(visible);
       }
   }
   ```
4. Call `update_pane_headers_visibility` inside `rebuild_projection_with` and `reset`.
5. Implement public mutator:
   ```rust
   pub fn set_pane_title_settings(&self, style: PaneTitleStyle, show_when_single: bool) {
       *self.pane_title_style.borrow_mut() = style;
       *self.pane_title_show_when_single.borrow_mut() = show_when_single;
       self.update_pane_headers_visibility();
   }
   ```

### FR-4: Global Window-Layer Broadcast (`src/ui/window.rs`)
1. Expose broadcasting function in `src/ui/window.rs`:
   ```rust
   pub fn apply_pane_title_settings_to_all_sessions(style: PaneTitleStyle, show_when_single: bool) {
       WIDGET_TO_SESSION.with(|m| {
           for session in m.borrow().values() {
               session.borrow().set_pane_title_settings(style, show_when_single);
           }
       });
   }
   ```

### FR-5: Preferences UI Controls (`src/ui/preferences.rs`)
1. In `TilixPreferencesWindow::new`, on the "Appearance" page (`page`), create an `adw::PreferencesGroup` titled `"Terminal Title"`:
   ```rust
   let title_group = adw::PreferencesGroup::new();
   title_group.set_title("Terminal Title");
   ```
2. Add `adw::ComboRow` for "Title Style":
   - Title: `"Title Style"`
   - Model options: `["Normal", "None"]`
   - Initial selection mapped from `current_config.borrow().pane_title_style`:
     - `PaneTitleStyle::Normal` $\rightarrow$ index `0`
     - `PaneTitleStyle::None` $\rightarrow$ index `1`
3. Add `adw::SwitchRow` for "Show title when single terminal":
   - Title: `"Show title when single terminal"`
   - Subtitle: `"Display terminal pane header bar even when no splits exist"`
   - Initial active state from `current_config.borrow().pane_title_show_when_single`.
4. In `sync_and_save`:
   - Read selected `pane_title_style` and switch state.
   - Update `cfg.pane_title_style` and `cfg.pane_title_show_when_single`.
   - Save configuration to disk via `cfg.save()`.
   - Dispatch `crate::ui::window::apply_pane_title_settings_to_all_sessions(cfg.pane_title_style, cfg.pane_title_show_when_single)`.
5. Connect `connect_selected_notify` and `connect_active_notify` signals to `sync_and_save`.

---

## 6. Non-Functional Requirements

### NFR-1: Headless Domain Testability
- All models, serialization logic, default values, and backwards compatibility checks must execute headlessly without an active display server.

### NFR-2: Backwards & Forwards Schema Compatibility
- Older `config.json` files from Phases 1 through 5 omitting `pane_title_style` or `pane_title_show_when_single` must deserialize seamlessly without error via `#[serde(default)]`.
- Unknown future JSON fields must be ignored without panics.

### NFR-3: Zero Widget Disruption & Flicker-Free Transitions
- Adjusting pane header visibility must mutate existing `TerminalPane` widgets in place without recreating terminal widgets, without interrupting child processes, and without shifting focus.

### NFR-4: GNOME HIG Compliance
- Preferences controls must use standard Libadwaita rows (`adw::ComboRow`, `adw::SwitchRow`, `adw::PreferencesGroup`) with clear descriptive labels.

### NFR-5: Zero Compilation Warnings & Regressions
- The entire codebase must compile cleanly under `cargo check --all-targets` and pass `cargo clippy --all-targets -- -D warnings`.
- Existing test suites (`test_phase2_domain`, `test_phase3_domain`, `test_phase4_packaging_polish`, `test_phase5_window_style_wide_handle`) must continue to pass 100%.

---

## 7. Validation & Test-First Strategy

1. **Unit Testing (`src/model/config.rs`):**
   - `test_pane_title_style_defaults`: Asserts `PaneTitleStyle::default() == PaneTitleStyle::Normal`.
   - `test_pane_title_style_serde`: Roundtrip serialization for `"normal"` and `"none"`.
   - `test_app_config_with_pane_title_defaults`: Verifies `AppConfig::default().pane_title_style == PaneTitleStyle::Normal` and `pane_title_show_when_single == true`.
   - `test_app_config_deserialize_v5_legacy_json`: Confirms legacy config JSON missing Phase 6 keys populates defaults cleanly.
2. **UI Component Unit Tests:**
   - In `src/ui/terminal_pane.rs`: Test `set_header_visible` and `is_header_visible`.
   - In `src/ui/session_view.rs`: Verify that `set_pane_title_settings` dynamically toggles header visibility, and that splitting a single pane with `show_when_single = false` makes both headers appear.
   - In `src/ui/window.rs`: Verify `apply_pane_title_settings_to_all_sessions`.
3. **Integration Test Suite (`tests/test_phase6_pane_toolbar.rs`):**
   - Headless integration tests validating JSON persistence, mutations, round-trip serialization, and compatibility.
4. **Validation Commands:**
   ```bash
   cargo check --all-targets
   cargo test --lib model::config
   cargo test
   cargo clippy --all-targets -- -D warnings
   ```
