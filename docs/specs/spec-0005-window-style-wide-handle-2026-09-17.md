# Master Specification: Phase 5 — Window Style Configuration & Splitter Wide Handle

- **Document ID:** `SPEC-0005`
- **Date:** 2026-09-17
- **Status:** Approved / Decision Ready
- **Workflow Level:** Medium/Large (Phase 5 of Tilix Rewrite)
- **Preceding Specification:** [`docs/specs/spec-0004-packaging-distribution-polish-2026-09-16.md`](file:///playground/tilix/docs/specs/spec-0004-packaging-distribution-polish-2026-09-16.md)
- **Target Location:** `/playground/tilix`

---

## 1. Context & Motivation

In the original Tilix terminal emulator, power users heavily customized their workspace geometry and visual hierarchy. Two quintessential configuration capabilities in original Tilix were:
1. **Window Style ("Hide Toolbar"):** The ability to hide the window header bar / toolbar completely to maximize terminal canvas space. In this mode, users interact with the terminal without window header chrome, relying on Tilix's rich keyboard shortcuts (e.g. `Ctrl+Shift+T` for new tab, `Ctrl+Shift+R` for split right, `Ctrl+Shift+D` for split down, `Ctrl+,` for preferences) while retaining clean GNOME HIG styling.
2. **"Use a wide handle for splitters":** The ability to toggle between standard subtle divider lines and wider grab handles for `gtk::Paned` splitters. While thin dividers maximize terminal screen real estate, wide handles significantly improve mouse ergonomics, accessibility, and touchscreen targeting for resizing adjacent terminal panes.

In the current Rust codebase (Phase 4), `TilixWindow` always instantiates and displays the `adw::HeaderBar`, and `SessionView` hardcodes `paned.set_wide_handle(true)` in `src/ui/session_view.rs` (line 543) without exposing user preferences.

Phase 5 addresses these limitations by elevating Window Style and Wide Handle into the pure headless domain model (`AppConfig`), exposing them in `TilixPreferencesWindow` under the Appearance tab, and providing real-time reactive projections in `TilixWindow` and `SessionView` without restarting the application or discarding active PTY sessions.

---

## 2. Goals & Non-Goals

### Goals
1. **Headless Domain Model Extensions:**
   - Define `WindowStyle` enum in `src/model/config.rs` with `Normal` (default) and `HideToolbar` variants.
   - Support Serde serialization and deserialization with `#[serde(rename_all = "snake_case")]` and backward-compatible defaults.
   - Extend `AppConfig` with `window_style: WindowStyle` and `use_wide_handle: bool` (defaulting to `false`, matching standard GNOME HIG thin dividers).
   - Maintain 100% backwards and forwards compatibility using `#[serde(default)]`.
2. **Interactive Preferences UI:**
   - In `src/ui/preferences.rs`, introduce a dedicated "Window" group in the "Appearance" page.
   - Add an `adw::ComboRow` for "Window Style" with selections for "Normal" and "Hide Toolbar".
   - Add an `adw::SwitchRow` for "Use a wide handle for splitters" with subtitle description.
   - Wire dynamic signal notifications to trigger persistence and live application updates.
3. **Reactive Window Style Projection (`TilixWindow`):**
   - In `src/ui/window.rs`, initialize `adw::HeaderBar` visibility based on `AppConfig::load().window_style`.
   - Maintain a weak-reference registry of active window header bars (`WINDOW_HEADER_BARS`) to broadcast window style changes across all open windows.
   - Guarantee that all window accelerators (`win.new-tab`, `win.preferences`, `win.split-right`, `win.split-down`, `win.close-pane`) remain 100% operational when the header bar is hidden.
4. **Reactive Wide Handle Projection (`SessionView`):**
   - Store `use_wide_handle: Rc<RefCell<bool>>` in `SessionView`.
   - Update `SessionView::build_node` to apply the configured wide handle state when constructing new `gtk::Paned` containers.
   - Implement `SessionView::set_wide_handle(&self, wide: bool)` using in-place recursive widget traversal of child `gtk::Paned` containers, preventing widget reparenting, terminal reload, or focus disruption.
   - Provide `apply_wide_handle_to_all_sessions(wide: bool)` in `src/ui/window.rs` leveraging the existing `WIDGET_TO_SESSION` registry.
5. **Headless Testing & Quality Standards:**
   - 100% headless test coverage for new domain models, default values, and JSON compatibility without requiring an X11/Wayland display server.
   - Zero compiler warnings with `cargo check --all-targets` and `cargo clippy --all-targets -- -D warnings`.

### Non-Goals
- Supporting legacy GTK3 server-side decorations (SSD) or window manager titlebar stripping (GTK4 / Libadwaita on Wayland uses client-side decorations).
- Per-pane or per-tab customization of wide handle (Tilix treats this as a global application preference).
- Overhauling Quake window styling (Quake drop-down window mode does not use standard window header bars).

---

## 3. Decision Gating Log

| Topic | Resolution | Source | Reason |
| :--- | :--- | :--- | :--- |
| **Window Style Variants** | `WindowStyle::Normal` and `WindowStyle::HideToolbar` | Decision | Matches original Tilix's primary window style option while adapting to GTK4/Libadwaita where CSD cannot be disabled on Wayland. |
| **Window Style Serialization** | `#[serde(rename_all = "snake_case")]` (`"normal"`, `"hide_toolbar"`) | Decision | Follows JSON naming conventions used throughout `AppConfig` and `Profile`. |
| **Wide Handle Default** | `use_wide_handle: false` | Decision | GTK4 `gtk::Paned` defaults to narrow dividers; false adheres to modern GNOME HIG minimalism while allowing users to opt into wide handles. |
| **Serde Backwards Compatibility** | Relies on existing `#[serde(default)]` on `AppConfig` | Fact | Existing config files omitting `window_style` and `use_wide_handle` automatically deserialize with default values. |
| **Window HeaderBar Registry** | `thread_local!` registry of `glib::WeakRef<adw::HeaderBar>` in `src/ui/window.rs` | Decision | Allows zero-coupling broadcast from preferences without memory leaks or holding dangling window pointers when windows close. |
| **Wide Handle Live Traversal** | Recursive widget traversal of `gtk::Paned` descendants in `SessionView` | Decision | Avoids rebuilding the projection or detaching widgets; updates divider widths in-place without terminal flickering or losing scroll state. |
| **Accelerator Retention** | Keyboard shortcuts registered via `app.set_accels_for_action` and `SimpleAction` on `adw::ApplicationWindow` | Fact | Hiding the `adw::HeaderBar` widget does not disable window-level actions or accelerators; `Ctrl+,` and `Ctrl+Shift+T` continue to function seamlessly. |
| **Preferences Group Placement** | New "Window" group on the "Appearance" page (`preferences-desktop-appearance-symbolic`) | Decision | Complies with GNOME HIG; groups visual presentation settings (terminal font/palette + window chrome/splitters) logically. |

---

## 4. Functional Requirements

### FR-1: Domain Model Extension (`src/model/config.rs` & `src/model/mod.rs`)
1. Define the `WindowStyle` enum:
   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
   #[serde(rename_all = "snake_case")]
   pub enum WindowStyle {
       #[default]
       Normal,
       HideToolbar,
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
   }
   ```
3. Update `Default for AppConfig`:
   - `window_style: WindowStyle::Normal`
   - `use_wide_handle: false`
4. Re-export `WindowStyle` in `src/model/mod.rs`:
   ```rust
   pub use config::{AppConfig, WindowStyle};
   ```

### FR-2: Preferences UI Controls (`src/ui/preferences.rs`)
1. In `TilixPreferencesWindow::new`, introduce a new `adw::PreferencesGroup` on `page` ("Appearance"):
   - Title: `"Window"`
2. Add `adw::ComboRow` for "Window Style":
   - Title: `"Window Style"`
   - Model options: `["Normal", "Hide Toolbar"]`
   - Initial selection mapped from `current_config.borrow().window_style`:
     - `WindowStyle::Normal` $\rightarrow$ index `0`
     - `WindowStyle::HideToolbar` $\rightarrow$ index `1`
3. Add `adw::SwitchRow` for "Use a wide handle for splitters":
   - Title: `"Use a wide handle for splitters"`
   - Subtitle: `"Increase draggable handle size between split panes"`
   - Initial state mapped from `current_config.borrow().use_wide_handle`
4. In `sync_and_save`:
   - Read `combo_row.selected()` and map back to `WindowStyle`:
     - `1` $\rightarrow$ `WindowStyle::HideToolbar`
     - `_` $\rightarrow$ `WindowStyle::Normal`
   - Read `switch_row.is_active()` for `use_wide_handle`.
   - Mutate `cfg.window_style` and `cfg.use_wide_handle`.
   - Save configuration to disk via `cfg.save()`.
   - Dispatch reactive broadcasts:
     ```rust
     crate::ui::window::apply_window_style_to_all_windows(cfg.window_style);
     crate::ui::window::apply_wide_handle_to_all_sessions(cfg.use_wide_handle);
     ```
5. Connect `connect_selected_notify` on the combo row and `connect_active_notify` on the switch row to trigger `sync_and_save`.

### FR-3: Window Style Reactive Projection (`src/ui/window.rs`)
1. Store a weak-reference registry of header bars:
   ```rust
   thread_local! {
       static WINDOW_HEADER_BARS: RefCell<Vec<glib::WeakRef<adw::HeaderBar>>> = RefCell::new(Vec::new());
   }
   ```
2. In `TilixWindow::new_empty`:
   - Load configuration: `let cfg = crate::model::AppConfig::load();`
   - Set initial visibility:
     ```rust
     header_bar.set_visible(cfg.window_style != crate::model::WindowStyle::HideToolbar);
     ```
   - Register header bar:
     ```rust
     WINDOW_HEADER_BARS.with(|bars| bars.borrow_mut().push(header_bar.downgrade()));
     ```
3. Expose broadcasting function:
   ```rust
   pub fn apply_window_style_to_all_windows(style: WindowStyle) {
       WINDOW_HEADER_BARS.with(|bars| {
           bars.borrow_mut().retain(|bar_weak| {
               if let Some(bar) = bar_weak.upgrade() {
                   bar.set_visible(style != WindowStyle::HideToolbar);
                   true
               } else {
                   false
               }
           });
       });
   }
   ```
4. Verify accelerator retention: Accelerators registered via `setup_accels` continue to fire on `AdwApplicationWindow` regardless of `header_bar` visibility.

### FR-4: Wide Handle Reactive Projection (`src/ui/session_view.rs`)
1. Add `use_wide_handle: Rc<RefCell<bool>>` to `SessionView`.
2. In `SessionView::with_model_and_dir`:
   - Initialize from saved config:
     ```rust
     let cfg = crate::model::AppConfig::load();
     let use_wide_handle = Rc::new(RefCell::new(cfg.use_wide_handle));
     ```
3. In `SessionView::build_node`:
   - Pass `wide_handle: bool` and apply to new split containers:
     ```rust
     paned.set_wide_handle(wide_handle);
     ```
4. Implement recursive in-place updater and public mutator on `SessionView`:
   ```rust
   fn set_paneds_wide_handle(widget: &gtk::Widget, wide: bool) {
       if let Ok(paned) = widget.clone().downcast::<gtk::Paned>() {
           paned.set_wide_handle(wide);
           if let Some(start) = paned.start_child() {
               Self::set_paneds_wide_handle(&start, wide);
           }
           if let Some(end) = paned.end_child() {
               Self::set_paneds_wide_handle(&end, wide);
           }
       }
   }

   pub fn set_wide_handle(&self, wide: bool) {
       *self.use_wide_handle.borrow_mut() = wide;
       let mut child = self.container.first_child();
       while let Some(c) = child {
           Self::set_paneds_wide_handle(&c, wide);
           child = c.next_sibling();
       }
   }
   ```
5. Expose broadcasting function in `src/ui/window.rs`:
   ```rust
   pub fn apply_wide_handle_to_all_sessions(wide: bool) {
       WIDGET_TO_SESSION.with(|m| {
           for session in m.borrow().values() {
               session.borrow().set_wide_handle(wide);
           }
       });
   }
   ```

---

## 5. Non-Functional Requirements

### NFR-1: Headless Domain Testability
- All models, serialization logic, default values, and schema compatibility checks must execute headlessly in CI without requiring an active X11 or Wayland display server or initializing GTK.

### NFR-2: Backwards & Forwards Schema Compatibility
- Older `config.json` files generated in Phase 1-4 that do not contain `window_style` or `use_wide_handle` must deserialize seamlessly without error, defaulting to `WindowStyle::Normal` and `use_wide_handle: false`.
- Any unexpected or future JSON properties must be ignored without panicking.

### NFR-3: GNOME Human Interface Guidelines (HIG) Compliance
- Preference rows must use standard Libadwaita styling (`adw::ComboRow`, `adw::SwitchRow`, `adw::PreferencesGroup`).
- Controls must present descriptive titles and subtitles.
- By default, splitters must use clean narrow handles matching standard GNOME applications, allowing users to explicitly opt into wide handles.

### NFR-4: Zero Compilation Warnings & Regressions
- The entire codebase must compile cleanly under `cargo check --all-targets` and pass `cargo clippy --all-targets -- -D warnings`.
- Existing test suites (`test_phase2_domain`, `test_phase3_domain`, `test_phase4_packaging_polish`) must continue to pass 100%.

---

## 6. Detailed File Specifications & Manifest

### 6.1 `src/model/config.rs`
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WindowStyle {
    #[default]
    Normal,
    HideToolbar,
}

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
}
```

### 6.2 `src/ui/preferences.rs`
```rust
// Window Group under Appearance Page
let window_group = adw::PreferencesGroup::new();
window_group.set_title("Window");
page.add(&window_group);

let window_style_names = ["Normal", "Hide Toolbar"];
let window_style_model = gtk::StringList::new(&window_style_names);
let window_style_row = adw::ComboRow::new();
window_style_row.set_title("Window Style");
window_style_row.set_model(Some(&window_style_model));
let style_idx = match current_config.borrow().window_style {
    WindowStyle::Normal => 0,
    WindowStyle::HideToolbar => 1,
};
window_style_row.set_selected(style_idx);
window_group.add(&window_style_row);

let wide_handle_row = adw::SwitchRow::new();
wide_handle_row.set_title("Use a wide handle for splitters");
wide_handle_row.set_subtitle("Increase draggable handle size between split panes");
wide_handle_row.set_active(current_config.borrow().use_wide_handle);
window_group.add(&wide_handle_row);
```

### 6.3 `src/ui/window.rs`
```rust
pub fn apply_window_style_to_all_windows(style: WindowStyle) {
    WINDOW_HEADER_BARS.with(|bars| {
        bars.borrow_mut().retain(|bar_weak| {
            if let Some(bar) = bar_weak.upgrade() {
                bar.set_visible(style != WindowStyle::HideToolbar);
                true
            } else {
                false
            }
        });
    });
}

pub fn apply_wide_handle_to_all_sessions(wide: bool) {
    WIDGET_TO_SESSION.with(|m| {
        for session in m.borrow().values() {
            session.borrow().set_wide_handle(wide);
        }
    });
}
```

---

## 7. Validation & Test-First Strategy

1. **Unit Testing (`src/model/config.rs`):**
   - `test_window_style_default`: Asserts `WindowStyle::default() == WindowStyle::Normal`.
   - `test_window_style_serde`: Verifies roundtrip serialization to `"normal"` and `"hide_toolbar"`.
   - `test_app_config_with_new_fields`: Verifies default values for `window_style` and `use_wide_handle`.
   - `test_app_config_backward_compatibility_v4_json`: Deserializes legacy JSON omitting `window_style` and `use_wide_handle` and confirms defaults are populated.
2. **UI Component Unit Tests:**
   - In `src/ui/session_view.rs`: Verify `session.set_wide_handle(true)` and `session.set_wide_handle(false)` dynamically toggle `paned.wide_handle()`.
   - In `src/ui/window.rs`: Verify `apply_window_style_to_all_windows` toggles header bar visibility.
3. **Integration Test Suite (`tests/test_phase5_window_style_wide_handle.rs`):**
   - Headless integration tests validating JSON persistence, mutations, round-trip serialization, and compatibility.
4. **Validation Commands:**
   ```bash
   cargo check --all-targets
   cargo test --lib model::config
   cargo test
   cargo clippy --all-targets -- -D warnings
   ```
