# Master Specification: Phase 2 — Sessions, Input Sync & Profiles Foundation

- **Document ID:** `SPEC-0002`
- **Date:** 2026-09-15
- **Status:** Approved / Decision Ready
- **Workflow Level:** Medium/Large (Phase 2 of Tilix Rewrite)
- **Preceding Specification:** [`docs/specs/spec-0001-rust-tilix-foundation-2026-09-15.md`](file:///playground/tilix/docs/specs/spec-0001-rust-tilix-foundation-2026-09-15.md)
- **Target Location:** `/playground/tilix`

---

## 1. Context & Motivation

Phase 1 established the core foundations of the Rust Tilix rewrite: a headless binary split tree (`LayoutTree`), asynchronous PTY child process spawning, reactive paned projection (`SessionView`), and Libadwaita window scaffolding (`TilixWindow`).

Phase 2 elevates Tilix from a single-session split terminal to a multi-session desktop powerhouse by delivering:
1. **Multi-Session Tab Management:** First-class multi-tab workflows leveraging Libadwaita's `AdwTabView` and `AdwTabBar`, where each tab encapsulates an independent `SessionView` with full keyboard switching and dynamic title synchronization.
2. **Synchronized Input Broadcast:** Tilix's hallmark productivity feature—broadcasting keyboard input simultaneously across terminal panes within a session, complete with session-wide toggles and per-pane overrides.
3. **Divider Ratio Persistence:** Addressing Phase 1 P2 Finding #1 by introducing addressable `SplitId`s and tracking `gtk::Paned` divider positions back to the domain model, preventing dividers from snapping back to 0.5 upon tree resplits.
4. **Color Schemes & Profiles Foundation:** A headless data model for terminal profiles and color palettes supporting Tilix/GNOME JSON palettes (16 ANSI colors, background, foreground, cursor), applied natively to `vte4::Terminal`.
5. **Session Layout Persistence:** JSON serialization and deserialization of session layouts, allowing users to save and restore custom multi-terminal split layouts with automatic ID remapping.

---

## 2. Goals & Non-Goals

### Goals
1. **Multi-Session Tab Architecture:** Integrate `adw::TabView` and `adw::TabBar` into `TilixWindow`, managing multiple independent `SessionView` tabs with dynamic title updates and GNOME HIG tab navigation shortcuts.
2. **Synchronized Keyboard Broadcasting:** Deliver zero-feedback, cycle-free keyboard input replication across terminal panes using `vte.connect_commit` and `vte.feed_child`, governed by session-level and pane-level sync flags.
3. **Persistent Split Divider Ratios:** Assign `SplitId` to split nodes in `LayoutTree`, connect `gtk::Paned`'s `notify::position` signal, and update the model's split ratio to preserve user-adjusted dividers across layout modifications.
4. **Headless Profile & Color Scheme Engine:** Create pure Rust data models for `Profile` and `ColorScheme` with JSON parsing for 16-color ANSI palettes, bundled presets, and native VTE color application.
5. **Layout Template Serialization:** Provide JSON serialization and deserialization for `SessionModel` / `LayoutTree` with safe ID remapping for importing templates into existing sessions.
6. **Headless Domain Testability:** Retain 100% test coverage for all new domain logic (theming, templates, split IDs, sync models) without requiring a display server.

### Non-Goals
- Wayland Drag-and-Drop for moving tabs between separate windows (deferred to Phase 3).
- Quake / Drop-down single-instance terminal mode via D-Bus IPC (deferred to Phase 3).
- System-wide GSettings schema compilation and preferences dialog UI (deferred to Phase 3/4).
- Custom badge rendering or notification banner overlays (deferred to Phase 3).

---

## 3. Functional Requirements

### FR-1: Multi-Session Tab Container (`AdwTabView` & `AdwTabBar`)
- `TilixWindow` must replace its singular central `SessionView` with an `adw::TabView`.
- An `adw::TabBar` must be embedded in `adw::ToolbarView` above the tab view, configured with `autohide = true` (hidden when only a single tab is open, shown when >= 2 tabs).
- Each tab must contain an isolated `SessionView` owning its own `SessionModel`, layout tree, and PTY processes.

### FR-2: Tab Actions & Keyboard Accelerators
- `win.new-tab` (`<Primary><Shift>t`): Creates a new tab containing a `SessionView` initialized with a single pane, selects the new tab, and focuses its terminal.
- `win.close-tab` / `win.close-pane` (`<Primary><Shift>w`):
  - If the active session has >1 pane, closes the focused pane.
  - If the active session has exactly 1 pane left, closes the tab.
  - If the last remaining tab in the window is closed, closes the window cleanly.
- Tab Switching:
  - `win.tab-next` (`<Primary>Page_Down`): Cycles to the next tab to the right (wrapping around).
  - `win.tab-prev` (`<Primary>Page_Up`): Cycles to the previous tab to the left (wrapping around).
  - `win.switch-tab-1` through `win.switch-tab-9` (`<Alt>1` .. `<Alt>9`): Instantly selects tab index 0 through 8.

### FR-3: Dynamic Tab Title Tracking
- Each `adw::TabPage` title must reflect the title of the active pane in its underlying `SessionView`.
- When terminal child processes emit OSC title change escape sequences (triggering `vte.connect_window_title_changed`), the corresponding pane updates its header label AND the parent tab page title updates immediately.
- When the active pane in a session changes (via click or directional navigation), the tab title updates to match the newly focused pane.

### FR-4: Session-Wide Input Synchronization
- A session-wide toggle must be accessible via shortcut `<Primary><Shift>i` (`win.toggle-sync-input`) and a header bar toggle button with icon `network-transmit-receive-symbolic`.
- When sync is enabled, typing in any synchronized pane in the active session broadcasts the keystrokes to all other synchronized panes in the session.
- The header bar toggle button must visually indicate active sync state (active toggle button styling).

### FR-5: Per-Pane Input Synchronization Override
- Each `TerminalPane` header bar must include an input sync toggle button (icon `input-keyboard-symbolic`).
- Clicking this button toggles `is_sync_enabled` for that specific pane.
- If a pane has sync disabled, it does not receive broadcast input from other panes, and input typed into it is not broadcast to others, even if session-wide sync is active.

### FR-6: Divider Ratio Tracking & Persistence (Fix for Phase 1 P2 Finding #1)
- `LayoutNode::Split` must include a unique `SplitId`.
- When `SessionView` projects split nodes into `gtk::Paned`, it tracks the `SplitId`.
- `SessionView` listens to `notify::position` on the `gtk::Paned`. When the user drags the divider, the relative ratio is computed:
  $$\text{ratio} = \left(\frac{\text{paned.position}()}{\text{paned.width()} \text{ or } \text{paned.height()}}\right).\text{clamp}(0.05, 0.95)$$
- The ratio is written back to `LayoutTree::set_split_ratio(split_id, ratio)`.
- When subsequent panes are split or closed, existing dividers maintain their user-dragged positions.

### FR-7: Color Scheme & Profile Subsystem
- The application must support parsing Tilix / GNOME JSON color schemes defining:
  - `name`: String
  - `foreground-color`: Hex color string (e.g. `"#c9cacc"`)
  - `background-color`: Hex color string (e.g. `"#1d1f21"`)
  - `cursor-background-color`: Hex color string (e.g. `"#ffffff"`)
  - `palette`: Array of 16 hex color strings (standard 16 ANSI terminal colors)
- Built-in color schemes must be available out-of-the-box: "Tilix Dark", "Tilix Light", "Solarized Dark", "Monokai".
- Applying a `ColorScheme` to a `TerminalPane` converts colors to `gdk::RGBA` and invokes `vte.set_colors()` and `vte.set_color_cursor()`.

### FR-8: Session Layout Persistence & Serialization
- `SessionLayoutTemplate` must serialize and deserialize to/from JSON via `serde`.
- An import routine `instantiate_session(&self, start_pane_id: u64) -> SessionModel` must remap all `PaneId`s and `SplitId`s sequentially to guarantee no collisions with existing sessions.
- Provide menu / action hooks `win.export-session-layout` and `win.import-session-layout`.

---

## 4. Non-Functional Requirements

### NFR-1: Headless Domain Testability
- All operations in `src/model/layout.rs`, `src/model/session.rs`, `src/model/theme.rs`, `src/model/profile.rs`, and `src/model/template.rs` must execute and pass in headless CI without `DISPLAY` or `WAYLAND_DISPLAY`.

### NFR-2: Cycle-Free, Zero-Feedback Input Synchronization
- Input broadcast must utilize `vte.feed_child()` on destination panes. Since `feed_child()` does not emit `commit` signals on destination widgets, there is zero risk of infinite reflection loops or deadlocks.

### NFR-3: GNOME HIG Compliance
- Tab view and tab bar must adhere strictly to GNOME HIG standards:
  - Tab bar autohides when a single tab exists.
  - Standard symbolic icons are used (`network-transmit-receive-symbolic`, `input-keyboard-symbolic`, `tab-new-symbolic`).
  - Standard tab navigation shortcuts (`<Primary>Page_Down`, `<Primary>Page_Up`, `<Alt>1`..`<Alt>9`).

### NFR-4: Memory Safety & PTY Cleanup on Tab Closure
- Closing an `adw::TabPage` must cleanly dispose of its `SessionView`, terminate all running child PTY processes in that session, drop terminal widgets, and remove all references from window memory.

---

## 5. Detailed Data Structures & API Design

### 5.1 Domain Model Updates (`src/model/`)

#### `src/model/layout.rs`
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SplitId(pub u64);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LayoutNode {
    Leaf(PaneId),
    Split {
        id: SplitId,
        orientation: SplitOrientation,
        ratio: f64,
        first: Box<LayoutNode>,
        second: Box<LayoutNode>,
    },
}

impl LayoutTree {
    pub fn new(initial_pane: PaneId) -> Self;
    pub fn next_split_id(&mut self) -> SplitId;
    pub fn split_with_id(
        &mut self,
        target: PaneId,
        orientation: SplitOrientation,
        new_pane: PaneId,
        split_id: SplitId,
    ) -> Result<(), LayoutError>;
    pub fn set_split_ratio(&mut self, split_id: SplitId, ratio: f64) -> bool;
    pub fn remap_ids(&mut self, next_pane: &mut u64, next_split: &mut u64);
}
```

#### `src/model/theme.rs`
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RgbColor {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: f64,
}

impl RgbColor {
    pub fn from_hex(hex: &str) -> Result<Self, ThemeError>;
    pub fn to_hex(&self) -> String;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColorScheme {
    pub name: String,
    pub comment: Option<String>,
    pub foreground: RgbColor,
    pub background: RgbColor,
    pub cursor: Option<RgbColor>,
    pub cursor_foreground: Option<RgbColor>,
    pub palette: [RgbColor; 16],
}

impl ColorScheme {
    pub fn from_json(json_str: &str) -> Result<Self, ThemeError>;
    pub fn to_json(&self) -> Result<String, ThemeError>;
    pub fn tilix_dark() -> Self;
    pub fn tilix_light() -> Self;
    pub fn solarized_dark() -> Self;
    pub fn monokai() -> Self;
}
```

#### `src/model/profile.rs`
```rust
use serde::{Deserialize, Serialize};
use crate::model::theme::ColorScheme;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CursorShapePreference {
    Block,
    IBeam,
    Underline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CursorBlinkPreference {
    System,
    On,
    Off,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub color_scheme: ColorScheme,
    pub font: Option<String>,
    pub scrollback_lines: Option<i64>,
    pub cursor_shape: CursorShapePreference,
    pub cursor_blink: CursorBlinkPreference,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            id: "default".into(),
            name: "Default".into(),
            color_scheme: ColorScheme::tilix_dark(),
            font: Some("Monospace 11".into()),
            scrollback_lines: Some(5000),
            cursor_shape: CursorShapePreference::Block,
            cursor_blink: CursorBlinkPreference::System,
        }
    }
}
```

#### `src/model/template.rs`
```rust
use serde::{Deserialize, Serialize};
use crate::model::layout::LayoutTree;
use crate::model::session::SessionModel;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionLayoutTemplate {
    pub name: String,
    pub description: Option<String>,
    pub layout: LayoutTree,
}

impl SessionLayoutTemplate {
    pub fn from_session(name: &str, session: &SessionModel) -> Self;
    pub fn instantiate_session(&self, start_pane_id: u64, start_split_id: u64) -> SessionModel;
    pub fn to_json(&self) -> Result<String, serde_json::Error>;
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error>;
}
```

### 5.2 UI Layer Updates (`src/ui/`)

#### `src/ui/terminal_pane.rs`
- Add input sync override button in header (`input-keyboard-symbolic`).
- Track `is_sync_enabled: Rc<Cell<bool>>`.
- Provide `connect_commit<F: Fn(PaneId, &str) + 'static>(&self, f: F)`.
- Provide `feed_child(&self, data: &[u8])`.
- Provide `apply_color_scheme(&self, scheme: &ColorScheme)`.
- Provide `apply_profile(&self, profile: &Profile)`.

#### `src/ui/session_view.rs`
- Track `sync_input_enabled: bool`.
- Wire `pane.connect_commit`:
  ```rust
  if session.sync_input_enabled && sender_pane.is_sync_enabled() {
      for (id, pane) in &session.panes {
          if *id != sender_id && pane.is_sync_enabled() {
              pane.feed_child(text.as_bytes());
          }
      }
  }
  ```
- In `build_node`, connect `paned.connect_notify(Some("position"))` with `SplitId`:
  ```rust
  let split_id = *id;
  paned.connect_notify(Some("position"), move |p, _| {
      let len = match p.orientation() {
          gtk::Orientation::Horizontal => p.width(),
          gtk::Orientation::Vertical => p.height(),
          _ => p.width(),
      };
      if len > 0 {
          let ratio = (p.position() as f64 / len as f64).clamp(0.05, 0.95);
          // dispatch update to session model
      }
  });
  ```
- Expose `title_changed_callback` for notifying tab headers of pane title updates.

#### `src/ui/window.rs`
- Embed `adw::TabView` and `adw::TabBar`.
- Maintain `tab_sessions: HashMap<adw::TabPage, Rc<RefCell<SessionView>>>`.
- Register window actions:
  - `win.new-tab` -> `<Primary><Shift>t`
  - `win.close-tab` / `win.close-pane` -> `<Primary><Shift>w`
  - `win.tab-next` -> `<Primary>Page_Down`
  - `win.tab-prev` -> `<Primary>Page_Up`
  - `win.switch-tab-1`..`win.switch-tab-9` -> `<Alt>1`..`<Alt>9`
  - `win.toggle-sync-input` -> `<Primary><Shift>i`
- Update header bar with sync input toggle button (`network-transmit-receive-symbolic`).

---

## 6. Validation & Test-First Strategy

### 6.1 Unit Tests (Headless Domain)
- `tests/test_layout_ratio_persistence`: Tests that `SplitId` assignments, ratio updates via `set_split_ratio()`, and tree balancing maintain exact ratios.
- `tests/test_theme_json`: Tests parsing of Tilix JSON themes, hex conversion, invalid hex rejection, and default presets.
- `tests/test_profile`: Tests profile creation, default settings, and serialization.
- `tests/test_template_persistence`: Tests exporting a 4-pane session into `SessionLayoutTemplate`, serializing to JSON, deserializing, remapping IDs, and asserting structural equality with zero collisions.
- `tests/test_session_sync_model`: Tests sync group state toggling, pane override state, and broadcast routing logic.

### 6.2 Validation Commands
```bash
cargo check --all-targets
cargo test --lib
cargo test
cargo clippy --all-targets -- -D warnings
cargo build
```
