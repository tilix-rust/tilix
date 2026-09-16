# Master Specification: Phase 3 — Wayland Quake, OSC CWD, DND & Interactive Preferences

- **Document ID:** `SPEC-0003`
- **Date:** 2026-09-15
- **Status:** Approved / Decision Ready
- **Workflow Level:** Medium/Large (Phase 3 of Tilix Rewrite)
- **Preceding Specification:** [`docs/specs/spec-0002-sessions-sync-profiles-2026-09-15.md`](file:///playground/tilix/docs/specs/spec-0002-sessions-sync-profiles-2026-09-15.md)
- **Target Location:** `/playground/tilix`

---

## 1. Context & Motivation

Phase 1 established the headless binary layout tree and asynchronous PTY child spawning. Phase 2 delivered multi-session tab management (`AdwTabView`/`AdwTabBar`), synchronized input broadcasting (`vte.connect_commit` / `vte.feed_child`), divider ratio persistence (`SplitId`), pure Rust color schemes/profiles, and layout template serialization.

Phase 3 addresses modern desktop usability and Wayland integration:
1. **OSC 7 Working Directory Inheritance:** When users split a pane or open a new tab, modern terminals must preserve the current directory of the active terminal rather than resetting to `$HOME`.
2. **Quake / Drop-Down Terminal Mode (Wayland-Native):** A persistent top-aligned terminal that drops down or conceals instantly via single-instance D-Bus CLI invocation (`tilix --quake-toggle`), respecting Wayland compositor ergonomics without fragile global X11 key grabs.
3. **OSC 777 & Process Completion Desktop Notifications:** Alerting users via `gio::Notification` when background builds, long-running tasks, or terminal bells complete in unfocused panes or background tabs.
4. **Wayland Drag-and-Drop (DND) & Tab Detaching:** Native tab detaching across windows via Libadwaita's `AdwTabView` create-window protocol, and visual pane reordering via GTK4 `DragSource` and `DropTarget` controllers coupled to pure domain pane swapping.
5. **Interactive Preferences Window (`AdwPreferencesWindow`):** A native Libadwaita preferences dialog allowing users to adjust color schemes, fonts, cursor shapes, and blink modes in real time across all open sessions.

---

## 2. Goals & Non-Goals

### Goals
1. **OSC 7 CWD Tracking & Inheritance:** Parse `vte.current_directory_uri_changed` into local filesystem paths and spawn child shells in the active directory for splits and new tabs.
2. **Wayland Quake Dropdown Window:** Implement single-instance CLI handling (`gio::ApplicationFlags::HANDLES_COMMAND_LINE`) supporting `--quake` and `--quake-toggle` via GLib D-Bus IPC, displaying a top-aligned, full-width window.
3. **Desktop Notifications:** Emit `gio::Notification` alerts when unfocused panes receive a terminal bell or child processes exit.
4. **Tab Detachment & Cross-Window Migration:** Implement `tab_view.connect_create_window` to support dragging tabs into new or existing windows.
5. **Wayland Pane Drag-and-Drop:** Equip pane headers with `GtkDragSource` and pane bodies with `GtkDropTarget`, swapping layout nodes in the pure domain `LayoutTree` and reparenting widgets without process interruption.
6. **Interactive Preferences Dialog:** Provide `win.preferences` (`<Primary>comma`) launching `AdwPreferencesWindow`, persisting settings to `~/.config/tilix/config.json`, and reactively applying updates across all active panes.
7. **100% Headless Testability:** Preserve 100% headless CI test coverage for all new domain logic (OSC 7 URI parsing, CLI parsing, layout pane swapping, configuration persistence).

### Non-Goals
- Distro packaging (Arch `PKGBUILD`, Flatpak `org.gnome.Tilix.json`) — deferred to Phase 4.
- Direct `wlr-layer-shell` foreign C protocol bindings (avoiding unstable C ABI dependencies; standard top-docked `AdwApplicationWindow` with window rules satisfies Wayland compositors).
- Custom badge overlay rendering on VTE surface (deferred until VTE termprop bindings stabilize).

---

## 3. Functional Requirements

### FR-1: OSC 7 Working Directory Tracking & Shell Inheritance
- The application must listen to `vte.connect_current_directory_uri_changed` on every `TerminalPane`.
- It must convert URI strings (e.g. `file://localhost/home/user/project` or `file:///home/user/project`) to canonical local paths using `parse_osc7_uri`.
- When splitting a pane (`win.split-right`, `win.split-down`, or header split buttons), the new pane's shell must be spawned with its working directory set to the target pane's current directory.
- When creating a new tab (`win.new-tab`), the new tab's initial pane must be spawned in the working directory of the active pane in the previously selected tab.

### FR-2: Wayland Quake / Drop-Down Mode via Single-Instance D-Bus IPC
- `TilixApplication` must set `gio::ApplicationFlags::HANDLES_COMMAND_LINE`.
- It must parse command-line arguments:
  - `--quake`: Launch or present the Quake window.
  - `--quake-toggle`: Toggle visibility of the Quake window (hide if visible and active; present if hidden).
  - `--preferences`: Open the preferences dialog.
- Secondary instances launching `tilix --quake-toggle` must communicate via D-Bus to the primary process and exit immediately with exit code 0.
- The Quake window (`TilixQuakeWindow`) must feature:
  - Top alignment and 100% monitor width.
  - Dedicated CSS class `.quake-window`.
  - Full support for multi-pane splits and tabs.

### FR-3: Desktop Notifications for Terminal Alerts & Process Completion
- `TerminalPane` must listen to `terminal.connect_bell` and `terminal.connect_child_exited`.
- When an alert/bell occurs, if the pane does not have focus or its parent window is inactive, a desktop notification is dispatched via `gio::Notification`:
  - Title: `"Terminal Alert"`
  - Body: Format with the pane's title (e.g. `"Bell received in build-server"`).
- When a child process terminates, if the pane is unfocused, a desktop notification is dispatched:
  - Title: `"Process Completed"`
  - Body: Format with terminal title and exit code status.

### FR-4: Drag-and-Drop Tab Detaching & Cross-Window Migration
- `TilixWindow` must wire `tab_view.connect_create_window`.
- When a user drags a tab outside the window bounds, Libadwaita invokes `create_window`.
- The handler creates a new `TilixWindow::new_empty(app)`, presents it, and returns `Some(new_win.tab_view().clone())`.
- Libadwaita reparents the tab page into the new window.
- The application coordinates session mapping across windows via `tab_view.connect_page_attached` and `tab_view.connect_page_detached`.

### FR-5: Drag-and-Drop Pane Swapping & Layout Tree Reordering
- `LayoutTree` must provide a pure domain method:
  `swap_panes(&mut self, a: PaneId, b: PaneId) -> Result<(), LayoutError>`
- `TerminalPane` headers must attach a `GtkDragSource` providing the `PaneId` as a `u64` value with action `gtk::gdk::DragAction::MOVE`.
- `TerminalPane` must attach a `GtkDropTarget` accepting `u64`.
- When a drop occurs, `SessionView` invokes `swap_panes` on its `SessionModel`, updates the layout tree, and rebuilds the projection, seamlessly swapping the visual positions of the two terminal panes without interrupting child shell processes.

### FR-6: Interactive Preferences Window (`AdwPreferencesWindow`)
- Register action `win.preferences` bound to shortcut `<Primary>comma`.
- The preferences window must provide:
  - Color Scheme combo row: "Tilix Dark", "Tilix Light", "Solarized Dark", "Monokai".
  - Cursor Shape combo row: "Block", "I-Beam", "Underline".
  - Cursor Blink combo row: "System", "On", "Off".
  - Font selection button/entry.
- Selecting a new value immediately updates the active `Profile` and reactively applies the settings to all active `TerminalPane`s in all sessions across all windows.

### FR-7: Persistent Application Configuration (`AppConfig`)
- Domain model `AppConfig` in `src/model/config.rs` must serialize and deserialize to JSON.
- It must persist to `~/.config/tilix/config.json`.
- Configuration changes made in the preferences window are saved to disk automatically.

---

## 4. Non-Functional Requirements

### NFR-1: Wayland Native Compliance & Zero X11 Dependency
- The application must run natively under Wayland (`GDK_BACKEND=wayland`) without relying on X11 atom properties or root window key grabs.
- Quake mode visibility toggling relies on standard compositor global shortcuts executing `tilix --quake-toggle` via D-Bus IPC.

### NFR-2: Headless Domain Testability
- All domain extensions (`parse_osc7_uri`, `parse_cli_args`, `LayoutTree::swap_panes`, `AppConfig` serialization) must run and pass in headless CI without a display server.

### NFR-3: Memory Safety, Cycle Prevention & Process Cleanup
- Tab detachment and cross-window transfers must maintain clean `Rc`/`Weak` reference semantics, preventing memory leaks and orphaned child processes.
- Closing detached windows or terminating detached tabs must cleanly kill running child PTY processes.

### NFR-4: GNOME HIG Compliance & Adaptive UI
- Preferences must be presented via `AdwPreferencesWindow` using standard Libadwaita rows and groups.
- Tab bar autohide and styling must follow standard GNOME 4x patterns.

---

## 5. Detailed Data Structures & API Design

### 5.1 Domain Layer

#### `src/pty/shell.rs` (OSC 7 URI Parsing)
```rust
use std::path::PathBuf;

/// Parses an OSC 7 file URI into a canonical local PathBuf.
/// Handles `file://localhost/path`, `file:///path`, and percent-encoded characters.
pub fn parse_osc7_uri(uri: &str) -> Option<PathBuf>;
```

#### `src/model/layout.rs` (Pane Swapping)
```rust
impl LayoutTree {
    /// Swaps the tree positions of two leaf panes while preserving split ratios and tree structure.
    pub fn swap_panes(&mut self, a: PaneId, b: PaneId) -> Result<(), LayoutError>;
}
```

#### `src/model/config.rs` (Application Configuration)
```rust
use serde::{Deserialize, Serialize};
use crate::model::Profile;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    pub default_profile: Profile,
    pub quake_height_percent: u32,
    pub quake_hide_on_unfocus: bool,
    pub notifications_enabled: bool,
    pub bell_notifications: bool,
    pub process_exit_notifications: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            default_profile: Profile::default(),
            quake_height_percent: 40,
            quake_hide_on_unfocus: false,
            notifications_enabled: true,
            bell_notifications: true,
            process_exit_notifications: true,
        }
    }
}

impl AppConfig {
    pub fn load() -> Self;
    pub fn save(&self) -> Result<(), std::io::Error>;
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error>;
    pub fn to_json(&self) -> Result<String, serde_json::Error>;
}
```

#### `src/app.rs` (CLI Action Parsing)
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliAction {
    NewWindow,
    QuakeShow,
    QuakeToggle,
    Preferences,
    Help,
    Version,
}

pub fn parse_cli_args<I: IntoIterator<Item = S>, S: AsRef<str>>(args: I) -> CliAction;
```

### 5.2 UI Layer

#### `src/ui/terminal_pane.rs`
```rust
impl TerminalPane {
    pub fn new(pane_id: PaneId, initial_directory: Option<&Path>) -> Self;
    pub fn current_directory(&self) -> Option<PathBuf>;
    pub fn connect_bell<F: Fn(PaneId) + 'static>(&self, f: F);
    pub fn setup_drag_source(&self);
    pub fn setup_drop_target<F: Fn(PaneId, PaneId) + 'static>(&self, on_swap: F);
}
```

#### `src/ui/session_view.rs`
```rust
impl SessionView {
    pub fn active_current_directory(&self) -> Option<PathBuf>;
    pub fn swap_panes(&self, a: PaneId, b: PaneId);
    pub fn split_pane_with_dir(&self, target: PaneId, orientation: SplitOrientation, dir: Option<&Path>);
}
```

#### `src/ui/quake.rs`
```rust
pub struct TilixQuakeWindow {
    window: adw::ApplicationWindow,
    session_view: Rc<RefCell<SessionView>>,
}

impl TilixQuakeWindow {
    pub fn new(app: &adw::Application) -> Self;
    pub fn toggle_visibility(&self);
    pub fn is_visible(&self) -> bool;
    pub fn present(&self);
    pub fn hide(&self);
}
```

#### `src/ui/preferences.rs`
```rust
pub struct TilixPreferencesWindow {
    window: adw::PreferencesWindow,
}

impl TilixPreferencesWindow {
    pub fn new<F: Fn(&Profile) + 'static>(parent: &gtk::Window, on_profile_changed: F) -> Self;
    pub fn present(&self);
}
```

#### `src/ui/notifications.rs`
```rust
pub struct NotificationService;

impl NotificationService {
    pub fn notify_bell(app: &adw::Application, pane_title: &str);
    pub fn notify_process_exit(app: &adw::Application, pane_title: &str, exit_code: i32);
}
```

---

## 6. Validation & Test-First Strategy

### 6.1 Headless Unit & Domain Tests
1. `test_parse_osc7_uri_standard`: Validates conversion of `file:///home/user/dir` to `PathBuf`.
2. `test_parse_osc7_uri_with_hostname`: Validates `file://localhost/home/user/dir`.
3. `test_parse_osc7_uri_percent_encoded`: Validates spaces (`%20`) and special characters.
4. `test_parse_osc7_uri_invalid`: Rejects non-file schemes and malformed URIs.
5. `test_cli_argument_parsing`: Validates `--quake`, `--quake-toggle`, `--preferences`, and empty argument list.
6. `test_layout_swap_panes`: Validates leaf swapping in 2-pane, 3-pane, and 4-pane trees without corrupting split IDs or ratios.
7. `test_layout_swap_invalid_panes`: Asserts errors on nonexistent pane IDs.
8. `test_app_config_json_roundtrip`: Verifies config serialization and default fallback values.

### 6.2 Validation Commands
```bash
cargo check --all-targets
cargo test --lib
cargo test
cargo clippy --all-targets -- -D warnings
cargo build
```
