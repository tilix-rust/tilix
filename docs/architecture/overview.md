# Tilix Rust Architecture Overview

**Status:** Living Architecture Document  
**Version:** 0.6.0 (Phase 6 Final Architecture)  
**Date:** 2026-09-17  

---

## 1. System Overview & Mission

Tilix is an advanced GTK-based tiling terminal emulator originally authored by Gerald Nunn in the D programming language using GTK3. While celebrated for its intuitive split-terminal workflow, native GNOME feel, and input synchronization, original Tilix suffered from severe architectural debt:
1. **Coupled Widget-as-State Model:** The GTK widget hierarchy *was* the session state. Tree manipulation (splitting, closing, rebalancing) manipulated live GTK3 containers directly, making headless unit testing impossible, serialization brittle, and layout transitions prone to widget layout bugs.
2. **Ecosystem Stagnation & Arch Deprecation:** D language tooling and libraries in desktop distributions waned. The D runtime and compilers (GDC/LDC/DMD) were removed from official Arch Linux repositories, resulting in the removal of Tilix from Arch Linux and making `pacman -S tilix` fail.
3. **Legacy Display Assumptions:** GTK3 abstractions lacked first-class Wayland ergonomics, modern Libadwaita adaptive design patterns, and GTK4 event controllers.

### Rust Tilix Mission
Rewrite Tilix in modern Rust using **GTK4**, **Libadwaita**, and **VTE4**, adhering to the following guiding tenets:
- **Headless Domain Core:** The terminal layout tree, session state, profiles, templates, CLI parsing, and configuration are pure, immutable-friendly algebraic data structures with zero dependencies on GTK, Wayland, or X11. They are 100% unit-testable in CI without a display server.
- **Unidirectional Reactive Projection:** The UI layer is a projection of the headless domain model. When the tree changes, the UI reconciles its `gtk::Paned` containers while preserving running `vte4::Terminal` instances.
- **Rock-Solid Process & PTY Lifecycle:** Terminal child processes are spawned asynchronously with strict signal and exit handling to prevent zombie processes and broken pipes.
- **Native GNOME HIG & Wayland Integration:** Full integration with Libadwaita styling, accent colors, header bars, adaptive tab bar with autohide, tab detaching across windows, Wayland pane drag-and-drop, and top-docked Quake dropdown mode via D-Bus IPC.
- **Zero-Feedback Synchronized Input:** Seamless input broadcast across terminal panes using `vte.connect_commit` and `vte.feed_child`.
- **Ecosystem Stagnation Resolved:** Native Arch Linux packaging (`PKGBUILD`), Flatpak distribution (`com.github.tilix_rust.json`), and Makefile install automation restore Tilix as a fully supported native package (`tilix-rust` providing `tilix`).

---

## 2. High-Level Architectural Block Diagram

```mermaid
flowchart TD
    subgraph Packaging_Distribution ["Packaging & Desktop Integration"]
        PKGBUILD["Arch Linux PKGBUILD<br/>(tilix-rust provides tilix)"]
        Flatpak["Flatpak Sandbox<br/>(build-aux/com.github.tilix_rust.json)"]
        Makefile["Standard Makefile<br/>(DESTDIR & PREFIX)"]
        Desktop["Desktop Entry<br/>(com.github.tilix_rust.desktop)"]
        AppStream["AppStream Metainfo<br/>(com.github.tilix_rust.metainfo.xml)"]
        Icon["Scalable SVG Icon"]
        Completions["Shell Completions<br/>(bash, zsh, fish)"]
    end

    subgraph IPC_Layer ["Single-Instance D-Bus CLI IPC"]
        CLI["CLI Invocation<br/>(tilix --quake-toggle)"]
        DBusService["D-Bus Service<br/>(com.github.tilix_rust.service)"]
        GApp["GApplication<br/>(HANDLES_COMMAND_LINE)"]
        CLI -.->|D-Bus IPC| GApp
        DBusService -.->|Activates| GApp
    end

    subgraph UI_Layer ["UI Layer (GTK4 + Libadwaita)"]
        Win["TilixWindow (Standard Window)"]
        QuakeWin["TilixQuakeWindow (Drop-Down Window)"]
        PrefWin["AdwPreferencesWindow (Preferences Dialog)"]
        TabBar["AdwTabBar (autohide = true)"]
        TabView["AdwTabView (Multi-Session & Tab DND)"]
        SV1["SessionView"]
        TP1["TerminalPane 1<br/>(Header Visibility + DND)"]
        TP2["TerminalPane 2<br/>(Header Visibility + DND)"]
    end

    subgraph Domain_Model ["Headless Domain Model (Pure Rust)"]
        SM["SessionModel"]
        LT["LayoutTree (swap_panes, set_split_ratio)"]
        Cfg["AppConfig (WindowStyle, WideHandle, PaneTitleStyle, ShowWhenSingle)"]
        Prof["Profile / ColorScheme"]
        OSC7["parse_osc7_uri(uri)"]
    end

    subgraph System_Integration ["System & Desktop Integration"]
        GNotif["GIO Desktop Notifications (Dynamic Titles)"]
        CWD["OSC 7 Working Directory Inheritance"]
    end

    Desktop --> GApp
    GApp -->|Normal Launch| Win
    GApp -->|--quake-toggle| QuakeWin
    GApp -->|--preferences| PrefWin
    Win --> TabBar
    Win --> TabView
    TabView <-->|Tab DND / Detach| Win
    TabView --> SV1
    QuakeWin --> SV1
    SV1 -->|Controls Header Visibility| TP1
    SV1 -->|Controls Header Visibility| TP2

    TP1 <-->|DND Pane Swap| TP2
    SV1 <-->|Syncs| SM
    SM --> LT
    PrefWin -.->|Updates Profile, Style, Wide Handle & Pane Title| Cfg
    Cfg -.->|Broadcast Profile, Wide Handle & Pane Title| SV1
    Cfg -.->|Broadcast Window Style| Win

    TP1 -->|connect_current_directory_uri_changed| OSC7
    OSC7 -->|Pass CWD to new splits/tabs| CWD
    TP1 -->|connect_bell / exit (Dynamic Title)| GNotif
```

---

## 3. Core Domain Model (Headless & Testable)

The domain model represents terminal arrangements as an algebraic binary tree. No GTK, GObject, or display types are referenced in this layer.

### 3.1 Data Structures
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PaneId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SplitId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitOrientation {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutTree {
    root: Option<LayoutNode>,
}
```

### 3.2 Tree Invariants & Operations
1. **Empty State & Leaves:** A `LayoutTree` can represent an active session or an empty state (`root: None`).
2. **Unique Identifiers:** Every `PaneId` and `SplitId` in the tree must be strictly unique.
3. **Splitting:** Replaces `Leaf(target)` with `Split { id, orientation, ratio: 0.5, first: Leaf(target), second: Leaf(new_pane) }`.
4. **Closing & Collapsing:** Finds the parent split of `Leaf(target)` and promotes the sibling branch.
5. **Balancing:** Recursively resets split ratios based on leaf counts.
6. **Dynamic Split Ratio Updates:** `set_split_ratio(split_id: SplitId, ratio: f64) -> bool` updates ratios dynamically.
7. **Pane Swapping (for DND):** `swap_panes(&mut self, a: PaneId, b: PaneId) -> Result<(), LayoutError>` swaps positions of two leaf panes while preserving tree topology.
8. **Geometric Directional Navigation:** Computes normalized 2D bounding boxes `[0.0, 1.0] x [0.0, 1.0]` of all leaves and ray-casts perpendicular intervals.

---

## 4. UI Projection & Event Flow

The UI layer maintains a pool of active `TerminalPane` widgets keyed by `PaneId`:
```rust
pub struct SessionView {
    container: gtk::Box,
    panes: Rc<RefCell<HashMap<PaneId, TerminalPane>>>,
    model: Rc<RefCell<SessionModel>>,
}
```
When `LayoutTree` changes (split, close, rebalance, swap):
1. The tree is traversed recursively.
2. For each `LayoutNode::Leaf(pane_id)`, the existing `TerminalPane` is retrieved from `panes`.
3. For each `LayoutNode::Split`, a `gtk::Paned` is allocated with the designated orientation and divider position.
4. Reparenting unparents running pane widgets before reattaching them, preserving running VTE instances, PTY connections, and scrollback.

---

## 5. PTY & VTE Integration

- **TerminalPane Composition:** Composite vertical `gtk::Box` wrapping a header bar (title, sync toggle, drag handle, split/close buttons) and a `vte4::Terminal`.
- **Shell Spawning & CWD Inheritance:** Spawns child shells asynchronously in the target directory (passed via OSC 7 or falling back to `$PWD` / `$HOME`).
- **Environment:** Sets `TERM=xterm-256color`, `COLORTERM=truecolor`.

---

## 6. Multi-Session Tab Management (`AdwTabView` & `AdwTabBar`)

- Each tab page wraps an isolated `SessionView`.
- The `AdwTabBar` is embedded in `ToolbarView::add_top_bar` with `autohide = true`.
- Dynamic title propagation binds terminal process titles to tab page titles.
- Tab detaching is wired via `tab_view.connect_create_window` to support dragging tabs into new windows.

---

## 7. Synchronized Input Broadcast Engine

- Interception via `connect_commit` on the active terminal.
- Cycle-free replication via `feed_child(bytes)` to eligible sibling panes.
- Weak references (`Rc::downgrade`) prevent circular memory leaks.

---

## 8. Profiles, Theming & Palette Subsystem

- Headless domain models `ColorScheme` and `Profile` in `src/model/theme.rs` and `src/model/profile.rs`.
- Supports 16-color ANSI palettes with background, foreground, and cursor colors.
- Interactive preferences via `AdwPreferencesWindow` with immediate reactive broadcast.

---

## 9. Split Ratio Persistence & Session Templates

- Addressable `SplitId` on split nodes.
- `gtk::Paned`'s `notify::position` signal writes divider positions back to the model.
- `SessionLayoutTemplate` serializes session structures to JSON with collision-free sequential ID remapping.

---

## 10. Wayland, Desktop Integration & GNOME HIG

- Zero X11 dependencies; native Wayland first.
- Adaptive Libadwaita styling and standard GNOME accelerators.

---

## 11. OSC 7 Working Directory Inheritance

- `parse_osc7_uri` converts `file://...` URIs to canonical local `PathBuf`s with percent-decoding.
- Splitting a pane or opening a new tab inherits the active directory.

---

## 12. Wayland Quake / Drop-Down Architecture

- Single-instance D-Bus IPC using `adw::Application` with `HANDLES_COMMAND_LINE`.
- Secondary CLI invocations (`tilix --quake-toggle`) forward to primary instance.
- Top-docked `TilixQuakeWindow` presents or hides without X11 global grabs.

---

## 13. GTK4 Event Controllers: Drag & Drop and Tab Detaching

- Tab detachment via `AdwTabView::connect_create_window`.
- Pane swapping via GTK4 `DragSource` on headers and `DropTarget` on containers hooked to `LayoutTree::swap_panes`.

---

## 14. Reactive Preferences & Desktop Notifications

- `AdwPreferencesWindow` provides interactive appearance, window, terminal title, behavior, and notification controls.
- Dedicated "Window" group on the "Appearance" page offers "Window Style" (`adw::ComboRow`) and "Use a wide handle for splitters" (`adw::SwitchRow`).
- Dedicated "Terminal Title" group on the "Appearance" page offers "Title Style" (`adw::ComboRow`: "Normal", "None") and "Show title when single terminal" (`adw::SwitchRow`).
- `AppConfig` persisted to `~/.config/tilix/config.json` with `#[serde(default)]`, ensuring backwards and forwards schema compatibility across releases.
- `NotificationService` dispatches desktop notifications for bell events and process completions with dynamic title lookup.

---

## 15. Distribution, Packaging & Desktop Integration Architecture

1. **Arch Linux Packaging (`PKGBUILD`):**
   - Compliant with Arch Linux Rust packaging guidelines (`cargo build --frozen --release`).
   - Declares `provides=('tilix')` and `conflicts=('tilix')`, restoring `pacman` installability.
2. **FreeDesktop Specifications:**
   - Desktop Entry (`com.github.tilix_rust.desktop`): Validated via `desktop-file-validate`, with desktop actions for New Window, Quake, and Preferences.
   - AppStream Metainfo (`com.github.tilix_rust.metainfo.xml`): Validated via `appstreamcli validate --no-net`.
   - D-Bus Service (`com.github.tilix_rust.service`): Enables D-Bus service activation.
   - Iconography: Vector SVG icon installed to `/usr/share/icons/hicolor/scalable/apps/`.
   - Completions: Shell completions installed for `bash`, `zsh`, and `fish`.
3. **Flatpak Sandbox:**
   - Manifest `build-aux/com.github.tilix_rust.json` targeting GNOME Platform 47 with appropriate terminal permissions.
4. **Makefile Automation:**
   - Standard `Makefile` supporting `DESTDIR` and `PREFIX` for reproducible system installation and package staging.

---

## 16. Window Style Configuration & Splitter Wide Handle Architecture (Phase 5)

Phase 5 introduces comprehensive configuration for terminal window chrome and splitter ergonomics, directly restoring key capabilities from original Tilix within modern GTK4/Libadwaita:

### 16.1 Domain Model & Compatibility
- **`WindowStyle` Enum:** Headless enum (`Normal`, `HideToolbar`) in `src/model/config.rs` with `#[serde(rename_all = "snake_case")]`. Default is `WindowStyle::Normal`.
- **Wide Handle Flag:** `use_wide_handle: bool` on `AppConfig`. Defaults to `false` in compliance with standard GNOME HIG thin divider ergonomics.
- **Backwards & Forwards Compatibility:** Rooted on `#[serde(default)]` on `AppConfig`, allowing legacy configs from Phase 1–4 omitting these fields to deserialize cleanly, while ignoring unexpected future attributes.

### 16.2 Reactive Window Style Projection (`TilixWindow`)
- **Header Bar Registry:** Thread-local `WINDOW_HEADER_BARS: RefCell<Vec<glib::WeakRef<adw::HeaderBar>>>` tracks active window header bars via weak references, eliminating memory leaks or dangling pointers on window closure.
- **Dynamic Toolbar Toggle:** `apply_window_style_to_all_windows(style)` dynamically toggles visibility across all open windows without restarting the application.
- **Accelerator Retention:** All window-level accelerators (`win.new-tab`, `win.preferences`, `win.split-right`, `win.split-down`, `win.close-pane`) registered on `adw::ApplicationWindow` remain 100% functional when the header bar is hidden.

### 16.3 Reactive Splitter Wide Handle Projection (`SessionView`)
- **State Storage:** `SessionView` maintains `use_wide_handle: Rc<RefCell<bool>>`, initialized from configuration upon construction.
- **Construction & In-Place Traversal:**
  - `build_node` propagates `wide_handle` to newly instantiated `gtk::Paned` widgets.
  - `set_wide_handle(&self, wide: bool)` performs in-place recursive traversal (`set_paneds_wide_handle`) over existing widget trees in `self.container`.
  - Avoids widget reparenting, terminal reloads, or PTY interruptions.
- **Session Broadcast:** `apply_wide_handle_to_all_sessions(wide)` broadcasts updates across all active sessions registered in `WIDGET_TO_SESSION`.

---

## 17. Terminal Pane Title & Toolbar Visibility Controls (Phase 6)

Phase 6 implements granular visibility controls for terminal pane header bars and titles, directly restoring Tilix's classic title style options and single-terminal title toggling.

### 17.1 Domain Model & Configuration Schema
- **`PaneTitleStyle` Enum:** Headless enum (`Normal`, `None`) defined in `src/model/config.rs` with `#[serde(rename_all = "snake_case")]` and defaulting to `PaneTitleStyle::Normal`.
  - Re-exported through `src/model/mod.rs` for public API consistency.
- **`AppConfig` Attributes:**
  - `pub pane_title_style: PaneTitleStyle`: Defaults to `PaneTitleStyle::Normal`.
  - `pub pane_title_show_when_single: bool`: Defaults to `true` (classic default where the pane header bar is visible even with a single terminal).
- **Schema Compatibility:** Full Serde roundtrip support and backward compatibility with legacy configuration versions (Phase 1–5), providing seamless fallback to defaults for missing keys and ignoring future attributes.

### 17.2 TerminalPane Header Visibility API
- **Header Container:** The pane header (`gtk::Box` containing title label, badge, close button, split buttons, and sync toggle) is encapsulated in `TerminalPane`.
- **Visibility Methods:**
  - `pub fn set_header_visible(&self, visible: bool)`: Updates header widget visibility via `self.header.set_visible(visible)`.
  - `pub fn is_header_visible(&self) -> bool`: Queries `self.header.get_visible()`.
- **Focus Preservation:** Header action buttons are non-focusable (`set_focusable(false)`), ensuring mouse interactions and visibility state changes never steal focus from the underlying `vte4::Terminal`.

### 17.3 Reactive Dynamic Projection (`SessionView`)
- **State Encapsulation:** `SessionView` maintains `pane_title_style: Rc<RefCell<PaneTitleStyle>>` and `pane_title_show_when_single: Rc<RefCell<bool>>`, initialized from `AppConfig::load()` upon session creation.
- **Header Visibility State Machine:**
  - Evaluated dynamically in `update_pane_headers_visibility(&self)`:
    - If `pane_title_style == PaneTitleStyle::None`: all pane headers are hidden (`visible = false`).
    - If `pane_title_style == PaneTitleStyle::Normal`:
      - If `pane_count <= 1`: `visible = pane_title_show_when_single`.
      - If `pane_count > 1`: `visible = true`.
  - Invoked automatically during `rebuild_projection(&self)`, ensuring that pane splits, closures, and resets re-evaluate visibility immediately without widget reparenting or PTY restarts.
- **Dynamic Setting Mutators:** `pub fn set_pane_title_settings(&self, style: PaneTitleStyle, show_when_single: bool)` updates session-local state and reapplies visibility across all active panes.

### 17.4 Global Broadcast & Preferences Integration
- **Broadcast Architecture:** `crate::ui::window::apply_pane_title_settings_to_all_sessions(style, show_when_single)` iterates across all active sessions registered in `WIDGET_TO_SESSION`, applying preferences instantly without application restart.
- **Interactive UI (`AdwPreferencesWindow`):**
  - "Terminal Title" preferences group on the "Appearance" page.
  - "Title Style" `adw::ComboRow` mapped to `Normal` (index 0) and `None` (index 1).
  - "Show title when single terminal" `adw::SwitchRow` bound to `pane_title_show_when_single`.
  - Changes instantly synchronize to disk (`config.json`) and broadcast to all live sessions.


