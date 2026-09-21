# Tilix Rust Architecture Overview

**Status:** Living Architecture Document  
**Version:** 0.16.0 (Phase 16 Dual Library Architecture & Test Compilation Optimization)  
**Date:** 2026-09-20  


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
        Win2["TilixWindow (Detached Window)"]
        QuakeWin["TilixQuakeWindow (Drop-Down Window)"]
        PrefWin["AdwPreferencesWindow (Preferences Dialog)"]
        TabBar["AdwTabBar (autohide = true)"]
        TabView["AdwTabView (Multi-Session & Tab DND)"]
        SV1["SessionView 1"]
        SV2["SessionView 2"]
        TP1["TerminalPane 1<br/>(Overlay + 5-Zone DND)"]
        TP2["TerminalPane 2<br/>(Overlay + 5-Zone DND)"]
        TP3["TerminalPane 3<br/>(Overlay + 5-Zone DND)"]
    end

    subgraph Domain_Model ["Headless Domain Model (Pure Rust)"]
        SM["SessionModel (adopt_pane, remove_pane)"]
        LT["LayoutTree (dock_pane, insert_pane_dock, calculate_dock_position)"]
        Cfg["AppConfig (WindowStyle, WideHandle, PaneTitleStyle, Keybindings)"]
        Prof["Profile / ColorScheme"]
        OSC7["parse_osc7_uri(uri)"]
    end

    subgraph System_Integration ["System & Desktop Integration"]
        GNotif["GIO Desktop Notifications (Dynamic Titles)"]
        CWD["OSC 7 Working Directory Inheritance"]
        DND_Reg["ACTIVE_PANE_DRAG (Thread-Local Process Registry)"]
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
    SV1 -->|Controls Header & Overlay| TP1
    SV1 -->|Controls Header & Overlay| TP2

    TP1 <-->|5-Zone Directional Docking & Reparenting| TP2
    TP2 -.->|Drag to Desktop Detach| Win2
    TP2 -.->|Cross-Session Drag| SV2
    SV2 --> TP3
    TP1 -.->|Registers Active Drag| DND_Reg
    DND_Reg -.->|Provides Transfer Context| Win2
    DND_Reg -.->|Provides Transfer Context| SV2

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DockPosition {
    Top,
    Bottom,
    Left,
    Right,
    Center,
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
7. **Pane Swapping (for Center Docking):** `swap_panes(&mut self, a: PaneId, b: PaneId) -> Result<(), LayoutError>` swaps positions of two leaf panes while preserving tree topology.
8. **Directional Docking (Intra-Tree):** `dock_pane(&mut self, source: PaneId, target: PaneId, position: DockPosition) -> Result<(), LayoutError>` excises source from the tree and redocks it adjacent to target or swaps if `Center`.
9. **Dynamic Dock Insertion (External):** `insert_pane_dock(&mut self, new_pane: PaneId, target: PaneId, position: DockPosition) -> Result<(), LayoutError>` docks an external pane into the tree.
10. **5-Zone Mathematical Docking Calculation:** `calculate_dock_position(x, y, w, h) -> DockPosition` computes normalized coordinates and evaluates center rectangle `[0.25, 0.75]` vs closest edge.
11. **Geometric Directional Navigation:** Computes normalized 2D bounding boxes `[0.0, 1.0] x [0.0, 1.0]` of all leaves and ray-casts perpendicular intervals.

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

- **TerminalPane Composition:** Wrapped in a root `gtk::Overlay` containing the composite vertical `gtk::Box` (header bar with title, sync toggle, drag handle, split/close buttons, and `vte4::Terminal`) and an overlay child (`.drop-indicator-overlay`) dynamically sized and positioned for 5-zone docking.
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
- Dedicated "Window" group on the "Appearance" page offers "Window Style" (`adw::ComboRow`), "Use a wide handle for splitters" (`adw::SwitchRow`), and "Show Tab Bar" (`adw::SwitchRow`).
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

---

## 18. TabBar Visibility & Shortcut Toggling Architecture

- **Persistent vs. Hidden TabBar Configuration:**
  - `show_tab_bar: bool` on `AppConfig` (defaults to `true`).
  - `tab_bar.set_autohide(false)` is maintained by default to prevent vertical viewport size shifts and bash prompt jumps when creating/closing tabs.
  - `tab_bar.set_visible(cfg.show_tab_bar)` reflects user configuration.
- **Reactive Projection & Window Registry:**
  - Thread-local `WINDOW_TAB_BARS` maintains `glib::WeakRef<adw::TabBar>` across all open windows.
  - `apply_show_tab_bar_to_all_windows(show: bool)` broadcasts visibility updates across all live application windows instantly.
- **Keyboard Shortcut & Action:**
  - Window action `win.toggle-tab-bar` toggles `cfg.show_tab_bar`, saves the preference to `config.json`, and broadcasts the change.
  - Default keybindings registered in `setup_accels`: `F12` (classic Tilix session/tab toggle key) and `<Primary><Shift>F12` (non-conflicting modifier shortcut).
- **Roadmap & Phase 7 Completion:**
  - Phase 7 delivered the Custom Keybinding Manager, replacing static accelerator maps with dynamic configuration-driven binding.

---

## 19. Custom Keybinding Manager & Dynamic Accelerator Architecture

Phase 7 introduces complete user customization of keyboard shortcuts, featuring a headless domain model, accelerator canonicalization, headless collision detection, live runtime rebinding, and a native Libadwaita preference interface with interactive keypress capture.

### 19.1 Headless Domain Model & Action Catalog (`src/model/keybindings.rs`)
- **Action Categories (`ActionCategory`):**
  - `SessionAndTabs`: New tab, close pane, close tab, tab switching, previous/next tab.
  - `SplitsAndLayout`: Split right, split down, balance layout, synchronize input.
  - `Navigation`: Focus terminal directional navigation (Up, Down, Left, Right).
  - `ViewAndSettings`: Toggle tab bar, preferences dialog.
- **Action Catalog (`ACTION_CATALOG`):**
  - Defines 24 standard actions with action ID, title, description, category, and default accelerators (`&'static [&'static str]`).
- **Configuration Schema (`KeybindingsConfig`):**
  - Encapsulates `custom: HashMap<String, String>` where keys are action IDs and values are accelerator strings.
  - Omitted or default bindings are omitted from the map, ensuring a minimal serialized configuration footprint.
  - Empty string `""` represents an explicitly disabled / unassigned shortcut.
- **Resolution API:**
  - `get_effective_accel(action_id)`: Resolves primary effective accelerator (custom override takes precedence over catalog default).
  - `get_all_effective_accels(action_id)`: Returns all active accelerator strings (supporting multi-accelerator defaults such as `F12` and `<Primary><Shift>F12`).
  - `is_customized(action_id)`: Reports whether user overrides exist.
  - `set_custom_accel`, `reset_action`, and `reset_all`.

### 19.2 Accelerator Normalization & Collision Detection
- **Pure Rust Normalization (`normalize_accelerator`):**
  - Parses modifier tokens (`<Primary>`, `<Shift>`, `<Alt>`, `<Super>`) with case-insensitivity.
  - Canonicalizes modifier aliases (`<Control>` and `<Ctrl>` map to `<Primary>`).
  - Emits canonical order `<Primary><Shift><Alt><Super>` followed by canonical key representation (e.g. `Up`, `Page_Down`, `F12`).
  - Zero dependencies on GTK display servers, enabling 100% headless CI testability.
- **Headless Conflict Detection (`check_conflict`):**
  - Compares candidate accelerators against all other actions' effective accelerators.
  - Ignores self-action bindings and empty/disabled shortcuts.
  - Returns `Option<ConflictInfo>` identifying colliding action ID, title, and accelerator.

### 19.3 Live Dynamic Application Rebinding (`src/ui/window.rs`)
- **Dynamic Accelerator Dispatch:**
  - `apply_keybindings_to_app(app, keybindings)`: Iterates over `ACTION_CATALOG` and updates `adw::Application` accelerator maps dynamically via `app.set_accels_for_action`.
  - `apply_keybindings_globally(keybindings)`: Dispatches accelerator updates across the default application instance without requiring application restart.
- **Startup Integration:**
  - `setup_accels(app)` delegates directly to `apply_keybindings_to_app(app, &AppConfig::load().keybindings)`.

### 19.4 Native Libadwaita Preferences UI (`src/ui/preferences.rs`)
- **"Shortcuts" Page (`adw::PreferencesPage`):**
  - Icon: `preferences-desktop-keyboard-shortcuts-symbolic`.
  - Top "Defaults" group with "Reset All Keybindings" action button.
  - Dedicated `adw::PreferencesGroup` for each `ActionCategory`.
  - Action rows (`adw::ActionRow`) featuring `gtk::ShortcutLabel` badges, "Edit" button, and contextual "Reset" button (visible only when customized).
- **Interactive Shortcut Capture Dialog (`ShortcutCaptureDialog`):**
  - Modal Libadwaita dialog with `gtk::EventControllerKey`.
  - Filters out standalone modifier keypresses (`Shift`, `Control`, `Alt`, `Super`).
  - Escape cancels the dialog; Backspace/Delete unbinds the shortcut.
  - Live preview with real-time conflict checking and warning banners.
  - "Set" / "Apply" and "Disable Shortcut" actions trigger config saving and global live rebinding.

---

## 20. Advanced Pane Drag-and-Drop, Directional Docking & Window Detach Architecture (Phase 8)

Phase 8 elevates Tilix's tiling ergonomics to parity with modern tiling IDEs and classic Tilix, introducing 5-zone directional docking, visual drop indicator overlays, live cross-session/cross-window pane transfers, and drag-to-desktop window detachment with zero child shell process interruption.

### 20.1 5-Zone Mathematical Docking Model (`src/model/layout.rs`)
- **`DockPosition` Enum:** Algebraic enumeration (`Top`, `Bottom`, `Left`, `Right`, `Center`) representing the five directional drop targets.
- **Zone Geometry (`calculate_dock_position(x, y, width, height)`):**
  - Normalizes pointer coordinates: $nx = \text{clamp}(x / w, 0.0, 1.0)$, $ny = \text{clamp}(y / h, 0.0, 1.0)$.
  - **Center Zone:** Defined by the bounding rectangle $[0.25, 0.75] \times [0.25, 0.75]$. If $0.25 \le nx \le 0.75$ and $0.25 \le ny \le 0.75$, evaluates to `DockPosition::Center` (triggering pane swap).
  - **Peripheral Zones:** For coordinates outside the center box, evaluates the minimum distance to the four outer boundaries:
    - $d_{\text{top}} = ny$
    - $d_{\text{bottom}} = 1.0 - ny$
    - $d_{\text{left}} = nx$
    - $d_{\text{right}} = 1.0 - nx$
  - The minimal distance unequivocally selects `Top`, `Bottom`, `Left`, or `Right`.
  - Degenerate dimensions ($w \le 0$ or $h \le 0$) fall back safely to `DockPosition::Center`.
  - Pure Rust implementation with zero GTK dependencies, enabling 100% headless CI testability.

### 20.2 Headless Layout Tree Docking & Dynamic Insertion
- **Intra-Tree Docking (`LayoutTree::dock_pane`):**
  - Replaces source pane with its sibling via `self.close(source)`.
  - Re-inserts source adjacent to target via `self.insert_pane_dock(source, target, position)`.
  - `DockPosition::Center` delegates directly to `self.swap_panes(source, target)`.
  - Docking onto self (`source == target`) is a safe no-op.
- **Dynamic Dock Insertion (`LayoutTree::insert_pane_dock`):**
  - Replaces `target` leaf with a new `LayoutNode::Split`:
    - `Left`: Split orientation `Horizontal`, `first: Leaf(new_pane)`, `second: Leaf(target)`.
    - `Right`: Split orientation `Horizontal`, `first: Leaf(target)`, `second: Leaf(new_pane)`.
    - `Top`: Split orientation `Vertical`, `first: Leaf(new_pane)`, `second: Leaf(target)`.
    - `Bottom`: Split orientation `Vertical`, `first: Leaf(target)`, `second: Leaf(new_pane)`.
  - Split ratio defaults to 0.5.
- **Session Model Synchronization (`SessionModel`):**
  - `remove_pane(id)`: Excises pane from `layout`, `sync_groups`, `pane_sync_overrides`, and `focus_history`, cleanly returning the fallback active pane according to MRU focus ordering.
  - `adopt_pane(new_pane, target, position)`: Integrates external pane into `layout`, adds it to `focus_history`, and marks it active.

### 20.3 Visual Drop Indicator Overlay (`src/ui/terminal_pane.rs`)
- **Overlay Hierarchy:** `TerminalPane` wraps its vertical container inside a `gtk::Overlay`.
  - Overlay child: `.drop-indicator-overlay` (`gtk::Box`).
  - Marked `can_target = false` to ensure pointer events pass through transparently to underlying drag and drop controllers.
- **Dynamic Geometry & Alignment:**
  - `show_drop_indicator(position)`:
    - `Top`: `halign = Fill`, `valign = Start`, size $(-1, H / 2)$.
    - `Bottom`: `halign = Fill`, `valign = End`, size $(-1, H / 2)$.
    - `Left`: `halign = Start`, `valign = Fill`, size $(W / 2, -1)$.
    - `Right`: `halign = End`, `valign = Fill`, size $(W / 2, -1)$.
    - `Center`: `halign = Fill`, `valign = Fill`, size $(-1, -1)$.
  - `hide_drop_indicator()`: Sets indicator visibility to `false`.
- **CSS Styling (`src/ui/window.rs`):**
  - Styled with `alpha(@accent_color, 0.35)` background and `2px solid @accent_color` border with rounded corners.

### 20.4 DND Protocol & Process-Wide Active Drag Registry (`src/ui/dnd.rs`)
- **Active Drag Registry:**
  - `ActivePaneDrag`: Encapsulates `pane_id`, weak reference `source_session_widget: glib::WeakRef<gtk::Widget>`, and cloned `pane: TerminalPane`.
  - Thread-local `ACTIVE_PANE_DRAG: RefCell<Option<ActivePaneDrag>>` tracks the pane in flight across windows.
- **`gtk::DragSource` Lifecycle:**
  - `prepare`: Sets `ACTIVE_PANE_DRAG` with the dragged pane and source session widget; provides `pane_id.0.to_value()`.
  - `drag_cancel`: Intercepts `gdk::DragCancelReason::NoTarget` (drop on desktop background) to trigger window detachment.
  - `drag_end`: Clears `ACTIVE_PANE_DRAG`.
- **`gtk::DropTarget` Lifecycle:**
  - `motion`: Checks if dragging over self (if so, hides overlay). Otherwise calculates `calculate_dock_position(x, y, w, h)`, activates `show_drop_indicator(pos)`, and returns `DragAction::MOVE`.
  - `leave`: Hides drop indicator overlay.
  - `drop`: Hides overlay, recalculates position, and fires `on_dock(src_id, dest_id, position)`.

### 20.5 Cross-Session & Cross-Window Reparenting (`src/ui/session_view.rs`)
- **Zero Process Interruption:**
  - `remove_pane_for_transfer(id)`: Removes pane from `panes` map and `model`, then calls `SessionView::detach_widget(pane.widget())`. The underlying `vte4::Terminal`, its master PTY, and child PID remain running and uninterrupted.
  - `pane.clear_callbacks()`: Clears all closures holding references to the source session, preventing stale closures or use-after-free.
  - `adopt_pane(pane, target_id, position)`: Inserts pane into destination `panes` map, wires UI callbacks to the target session, inserts into `model`, and invokes `rebuild_projection()`.
- **Automatic Empty Session Cleanup:**
  - If a pane transfer leaves the source session empty (`is_empty()`), the source window automatically closes the corresponding tab page via `crate::ui::window::close_session_tab`. If that was the last tab, the source window closes automatically.

### 20.6 Window Detachment on Desktop Drop (`src/ui/window.rs`)
- **Single-Pane Safety Guard:**
  - `detach_drag_to_new_window` checks the total pane count across all tabs in the source window. If `total_panes <= 1`, detachment is aborted, preventing accidental destruction of the user's sole terminal window.
- **Detached Window Spawning:**
  - Excises the pane via `source_session.remove_pane_for_transfer(pane_id)`.
  - Instantiates a new empty window via `TilixWindow::new_empty(&app)`.
  - Mounts the existing pane via `create_tab_with_existing_pane(pane)` without restarting the shell.
  - Presents the newly detached window to the user.

---

## 21. Profile Customization Subsystem (Phase 9 Architecture)

### 21.1 Extended Profile Schema & Preference Enums (`src/model/profile.rs`)
- **Full Upstream Parity:** Models all 7 tabs from upstream `com.gexperts.Tilix.Profile` schema:
  - **General:** `terminal_title`, dimensions (`columns`, `rows`), `cell_width_scale` and `cell_height_scale` (clamped 1.0..2.0), `draw_margin`, `text_blink_mode`, `allow_bold`, `rewrap_on_resize`, `use_system_font`, `font`, `select_by_word_chars`, `cursor_shape`, `cursor_blink`, `terminal_bell`.
  - **Command:** `login_shell`, `use_custom_command`, `custom_command`, `exit_action`.
  - **Color:** `color_scheme`, `use_theme_colors`, `background_transparency_percent`, `dim_transparency_percent`, bold color overrides, `bold_is_bright`, cursor color overrides, highlight color overrides.
  - **Scrolling:** `show_scrollbar`, `scroll_on_output`, `scroll_on_keystroke`, `scrollback_unlimited`, `scrollback_lines`.
  - **Compatibility:** `backspace_binding`, `delete_binding`, `encoding`, `cjk_utf8_ambiguous_width`.
  - **Badge:** `badge_text`, `badge_position`, badge color overrides, badge font overrides.
  - **Advanced:** `automatic_switch` rules, `custom_hyperlinks`, `triggers`, silence notification configuration.
- **Preference Enums & Conversions:**
  - `EraseBindingPreference`: `Auto`, `AsciiDelete`, `AsciiBackspace`, `DeleteSequence`, `Tty` with zero-cost conversion to `vte4::EraseBinding`.
  - `TextBlinkModePreference`: `Never`, `Focused`, `Unfocused`, `Always` with conversion to `vte4::TextBlinkMode`.
  - `TerminalBellPreference`: `None`, `Sound`, `Icon`, `IconSound`.
  - `ExitActionPreference`: `Close`, `Restart`, `Hold`.
  - `CjkWidthPreference`: `Narrow` (1 cell), `Wide` (2 cells).
  - `BadgePosition`: `Northwest`, `Northeast`, `Southwest`, `Southeast`.

### 21.2 Multi-Profile CRUD & Backward Compatibility (`src/model/config.rs`)
- **Collection Management:**
  - `profiles: Vec<Profile>`: Stores all user-defined profiles.
  - `default_profile_id: String`: Designates the default profile.
  - `default_profile: Profile`: Retained and synchronized with `default_profile_id` for 100% backward compatibility with legacy serde and Phase 1-8 tests.
- **CRUD Operations:**
  - `get_profile(id)` & `get_profile_mut(id)`: Lookup by ID.
  - `get_default_profile()`: Returns the current active default.
  - `add_profile(profile)`: Adds a new profile with collision detection and automatic ID generation.
  - `duplicate_profile(id)`: Clones an existing profile, generating a unique ID and `(Copy)` naming suffix.
  - `delete_profile(id)`: Removes profile with a guard enforcing `CannotDeleteLastProfile`. If deleting the default profile, automatically promotes the first remaining profile as default.
  - `set_default_profile(id)`: Switches active default.
  - `update_profile(profile)`: In-place update with automatic synchronization of `default_profile`.
- **Legacy Normalization:**
  - `AppConfig::from_json` automatically populates `profiles` and `default_profile_id` from legacy JSON payloads lacking multi-profile fields.

### 21.3 Token Expansion & Automatic Switching Engine (`src/model/profile.rs`)
- **Pure Domain Token Engine:**
  - `expand_tokens(format_str, ctx)` supports `${id}`, `${title}`, `${profile}`, `${directory}`, `${appName}` with zero GTK dependencies.
  - `expand_title_format` and `expand_badge_format` provide dedicated interfaces for header titles and badge overlays.
- **Automatic Profile Switching:**
  - `ProfileSwitchRule`: Encapsulates `hostname`, `directory`, and target `profile_id`.
  - `ProfileSwitchRule::matches`: Evaluates hostname (exact or `*` wildcard, case-insensitive) and directory (exact match, prefix match, or `*` wildcard).

### 21.4 PTY Shell Spawning & Login Shells (`src/pty/shell.rs`)
- **Login Shell Prefixing:**
  - `format_shell_argv0(shell_path, login_shell)`: Converts `/bin/bash` with `login_shell: true` into `"-bash"` so the shell invokes login profiles (`/etc/profile`, `~/.bash_profile`).
- **Custom Command Execution:**
  - `build_spawn_args(profile, detected_shell)`: If `use_custom_command` is enabled with a non-empty command, returns `("/bin/sh", ["/bin/sh", "-c", command])`. Otherwise falls back to detected shell with formatted argv0.

### 21.5 Dynamic TerminalPane Profile Integration (`src/ui/terminal_pane.rs`, `src/ui/window.rs`)
- **Scrollbar Widget:** External `gtk::Scrollbar` bound to `terminal.vadjustment()` inside a horizontal layout box; toggles visibility dynamically according to `profile.show_scrollbar`.
- **Badge Overlay:** `gtk::Label` with `.terminal-badge` mounted non-targetable inside `self.overlay`, rendered using token-expanded text and aligned to `BadgePosition`.
- **Margin Line:** Non-targetable vertical guide overlay with `.terminal-margin-line` visible when `draw_margin > 0`.
- **Unfocused Dimming:** `set_active(false)` applies `1.0 - (dim_transparency_percent / 100.0)` opacity to `vte::Terminal`.
- **Exit Action Handling:** `connect_child_exited` handles `Close` (destroys pane), `Restart` (re-spawns shell in place), and `Hold` (preserves terminal buffer and annotates title with `[Process exited: code]`).
- **Reactive Profile Application:** `apply_profile` reconfigures live terminals dynamically without restarting running processes.

### 21.6 Historical Libadwaita Profiles Preferences Editor (`src/ui/preferences.rs`)
- **Initial Phase 9 Design:** Combobox selecting from `config.profiles`, with New, Duplicate, Delete (guarded), and Set Default actions.
- **7 Organized Tabs:** General, Command, Color (with transparency and palette overrides), Scrolling, Compatibility, Badge, and Advanced automation.
- **Instant Reactive Persistence:** Every input modification saves immediately to configuration and pushes updates to all running terminal sessions via `apply_profile_to_all_sessions`.

---

## 22. Profile Preferences UI Parity Subsystem (Phase 10 Architecture)

### 22.1 Visual & Structural Parity Architecture (`src/ui/preferences.rs`)
- **Libadwaita Row Deprecation:** Replaced generic Libadwaita row containers (`ActionRow`, `SwitchRow`, `SpinRow`, `ComboRow`) and the "Settings Section" dropdown in the Profile page with native GTK4 `gtk::Grid`, `gtk::Box`, and standard GTK4 controls to achieve 100% pixel-and-layout parity with original Tilix screenshots.
- **Profile Management Header Bar:**
  - Placed persistently at the top of the Profiles preference page:
  - `Profile:` bold label.
  - `gtk::DropDown` populated with profile names, updating selection upon switching.
  - `[ New ]` button adding a fresh profile and selecting it.
  - `[ Duplicate ]` button cloning the active profile with `(Copy)` naming.
  - `[ Delete ]` button styled `.destructive-action`, guarded against deleting the final profile.
  - `[ Set as Default ]` button promoting the active profile to application-wide default.

### 22.2 Canonical 7-Tab Notebook Structure
- **gtk::Notebook Integration:** The profile editor mounts directly under the top header bar with 7 canonical tabs:
  1. **General:** Two-column grid with right-aligned labels.
     - `Profile name`: `gtk::Entry`.
     - `Terminal title`: Entry with `pan-down-symbolic` token popover presets (`${id}: ${title}`, `${title}`, `${profile}`, `${directory}`, `${appName}`).
     - `Terminal size`: Columns and rows spin buttons with dedicated `[ Reset ]` button restoring 80x24.
     - `Cell spacing`: Width and height spin buttons (1.0..2.0) with dedicated `[ Reset ]` button restoring 1.0x1.0.
     - `Margin`: Spin button (0..500 px).
     - `Text blink mode`: Dropdown (`Never`, `Focused`, `Unfocused`, `Always`).
     - `Custom font`: CheckButton coupled to `gtk::FontButton` sensitivity.
     - `Word-wise select chars`: Entry for custom word delimiters.
     - `Cursor`: Shape (`Block`, `I-Beam`, `Underline`) and blink mode (`System`, `On`, `Off`).
     - `Terminal bell`: Dropdown (`None`, `Sound`, `Icon`, `Icon and sound`).
  2. **Command:**
     - `Run command as a login shell`: CheckButton toggling argv0 prefixing.
     - `Run a custom command instead of my shell`: CheckButton controlling sensitivity of indented custom command entry.
     - `When command exits`: Dropdown (`Exit the terminal`, `Restart the command`, `Hold the terminal open`).
  3. **Color:**
     - Scheme selector dropdown (`Tilix Dark`, `Tilix Light`, `Solarized Dark`, `Monokai`, `Custom`) and `[ Export ]` button saving to user schemes directory.
     - 2-column, 18-button color palette grid matching upstream layout (Background, Black, Red, Green, Orange, Foreground, Blue, Purple, Turquoise, Grey).
     - Color customization reactivity: Editing any palette or foreground/background color immediately switches `color_scheme` to `"Custom"`.
     - Options section: `Use theme colors` CheckButton with `[ Advanced v ]` popover for Bold, Cursor, and Highlight color overrides; `Show bold text in bright colors` CheckButton.
     - Transparency & Unfocused Dim: Horizontal `gtk::Scale` sliders (0..100) with right-aligned labels.
  4. **Scrolling:**
     - CheckButtons for `Show scrollbar`, `Scroll on output`, and `Scroll on keystroke`.
     - `Limit scrollback to:` CheckButton coupled to SpinButton sensitivity; unchecking configures `scrollback_unlimited = true`.
  5. **Compatibility:**
     - Dropdowns for `Backspace key generates` and `Delete key generates` (`Automatic`, `Control-H`, `ASCII DEL`, `Escape sequence`, `TTY`).
     - `Encoding` dropdown (`UTF-8 Unicode`, `ISO-8859-1`, `Windows-1252`, `US-ASCII`).
     - `Ambiguous-width characters` dropdown (`Narrow`, `Wide`).
  6. **Badge:**
     - Badge text entry with token presets popover (`${directory}`, etc.).
     - `Badge position` dropdown (`Northwest`, `Northeast`, `Southwest`, `Southeast`).
     - Custom font CheckButton with coupled `gtk::FontButton` sensitivity.
  7. **Advanced:**
     - `Notify New Activity`: Enable by default CheckButton and silence threshold spin button.
     - `Custom Links`: Management dialog trigger.
     - `Automatic Profile Switching`: Framed ScrolledWindow with rule list (`hostname:directory`) and modal dialogs for `[ Add ]`, `[ Edit ]`, and `[ Delete ]`.

### 22.3 Reactive State Synchronization
- Every widget modification immediately mutates `current_config`, writes asynchronously to disk (`save()`), triggers `on_profile_changed()` to broadcast live updates to running terminal panes via `apply_profile_to_all_sessions()`, and updates the dropdown state without refcell borrow conflicts.

---

## 23. Title Options & Token Subsystem (Phase 11)

### 23.1 Token Engine & Expansion Model (`src/model/title.rs`)
- **Headless Domain Expansion:** Headless pure token expansion engine isolated from GTK/display servers.
- **TitleEditScope:** Differentiates token catalogs across contexts:
  - `TitleEditScope::Terminal`: Supports terminal tokens (`${title}`, `${id}`, `${profile}`, `${directory}`, `${process}`, `${user}`, `${host}`, `${cols}`, `${rows}`, etc.).
  - `TitleEditScope::Session`: Supports session tokens (`${title}`, `${sessionName}`, `${terminalCount}`, `${profile}`, `${directory}`, `${appName}`).
  - `TitleEditScope::Window`: Supports window tokens (`${appName}`, `${sessionName}`, `${sessionNumber}`, `${sessionCount}`, `${activeTitle}`).
  - `TitleEditScope::Badge`: Supports badge tokens (`${directory}`, `${user}`, `${host}`, `${cols}`, `${rows}`, `${sessionName}`).
- **TokenContext:** Captures runtime state snapshot:
  - Terminal pane info: `terminal_id`, `terminal_title`, `process_name`, `directory`, `user`, `host`, `profile_name`, `cols`, `rows`.
  - Session info: `session_name`, `session_number`, `session_count`, `terminal_count`.
  - Window info: `app_name`, `active_title`.
- **Interpolation Grammar:**
  - Token syntax: `${var}` or `${var:fallback}`. Fallback values are parsed and substituted when the variable evaluates to empty.
  - Case-insensitive token identifiers.
  - Safe replacement preventing recursive expansion and preserving unknown tokens.

### 23.2 Configuration Extension (`src/model/config.rs`)
- **Default Session Name:** `AppConfig.default_session_name: String`, defaulting to `"${title}"`.
- **Application Title:** `AppConfig.app_title: String`, defaulting to `"${appName}: ${sessionName}"`.
- Fully backwards compatible with existing config files via `#[serde(default = "...")]`.

### 23.3 Scoped Token Popover Menus (`src/ui/preferences.rs`)
- Replaces static popovers with `create_scoped_token_menu_button(target_entry, scope)`.
- Categorized menu sections:
  - **Terminal / Session / Window Variables:** Context-relevant token action buttons.
  - **Help / Presets Section:** Standard default patterns.
- Direct cursor-position insertion into `gtk::Entry` via `target_entry.insert_text(tok, &mut pos)` with focus retention.
- Integrated into:
  - Profile preferences: Terminal title entry (`TitleEditScope::Terminal`) and Badge entry (`TitleEditScope::Badge`).
  - Appearance preferences: Default session name entry (`TitleEditScope::Session`) and Application title entry (`TitleEditScope::Window`).

### 23.4 Reactive Dynamic Title Synchronization (`src/ui/session_view.rs`, `src/ui/window.rs`)
- **Session dynamic title sync:** `SessionView` observes terminal pane title changes and active pane focus changes. Computes `compute_session_title()` from `AppConfig.default_session_name` and updates tab page title (unless custom overridden by user).
- **Window dynamic title sync:** `TilixWindow` registers window updater closure in `WINDOW_TITLE_UPDATERS`. Reacts to active tab switch, session title changes, and preference changes via `apply_title_settings_to_all_windows()`.
- Updates both the Libadwaita window title (`window.set_title`) and `adw::WindowTitle` (`title_widget.set_title` / `set_subtitle`).
- Safe borrow checking (`try_borrow`) prevents re-entrant RefCell panics during tab/pane split and closure lifecycles.

---

## 24. Window Geometry & Dynamic Sizing (Phase 12)

### 24.1 Character Cell Measurement (`src/ui/geometry.rs`)
- **Headless VTE Measurement:** Character cell dimensions are accurately measured using a headless `vte::Terminal` instance configured with the profile's font (or system fallback `"Monospace 11"`) and cell scale factors (`cell_width_scale`, `cell_height_scale`).
- **Metric Verification:** Cell dimensions are extracted via `char_width()` and `char_height()`. If font measurement fails or returns non-positive dimensions, the system falls back gracefully to `(DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT)` (900x600).

### 24.2 Geometry Decomposition & Summation Formula
Target window dimensions are computed from the profile's character grid and active window chrome:
- **Terminal Grid Width:**
  $$W_{\text{term}} = (C \times W_{\text{cell}}) + W_{\text{scrollbar}} + (2 \times \text{profile.draw\_margin})$$
  where $C = \max(1, \text{default\_size\_columns})$, and $W_{\text{scrollbar}} = 16$ if `profile.show_scrollbar` else 0.
- **Terminal Grid Height:**
  $$H_{\text{term}} = (R \times H_{\text{cell}}) + H_{\text{pane\_header}}$$
  where $R = \max(1, \text{default\_size\_rows})$, and $H_{\text{pane\_header}} = 36$ if `pane_title_style != PaneTitleStyle::None && pane_title_show_when_single` else 0.
- **Window Chrome Summation:**
  $$W_{\text{chrome}} = 0$$
  $$H_{\text{chrome}} = H_{\text{header\_bar}} + H_{\text{tab\_bar}}$$
  where $H_{\text{header\_bar}} = 46$ if `window_style != WindowStyle::HideToolbar` else 0, and $H_{\text{tab\_bar}} = 38$ if `show_tab_bar` else 0.
- **Raw Dimensions:**
  $$W_{\text{raw}} = W_{\text{term}} + W_{\text{chrome}}$$
  $$H_{\text{raw}} = H_{\text{term}} + H_{\text{chrome}}$$

### 24.3 Screen Boundary Clamping & Headless Fallbacks
- **Monitor Boundary Clamping:** When a target `gdk::Monitor` is available, dimensions are clamped to a maximum ratio of 90% (`MAX_MONITOR_RATIO = 0.90`):
  $$W_{\text{max}} = \text{round}(W_{\text{mon}} \times 0.90), \quad H_{\text{max}} = \text{round}(H_{\text{mon}} \times 0.90)$$
  $$W_{\text{final}} = \text{clamp}(W_{\text{raw}}, \min(300, W_{\text{max}}), W_{\text{max}})$$
  $$H_{\text{final}} = \text{clamp}(H_{\text{raw}}, \min(200, H_{\text{max}}), H_{\text{max}})$$
- **Minimum Bounds:** Ensures the window never collapses below `MIN_WINDOW_WIDTH` (300px) by `MIN_WINDOW_HEIGHT` (200px).
- **Headless Fallback:** In headless CI or when cell dimensions cannot be resolved, standard fallback `(900, 600)` is applied cleanly without runtime warnings or panics.

### 24.4 Window Creation & Detached Tab Integration (`src/ui/window.rs`)
- **Default Profile Sizing:** `TilixWindow::new_empty` delegates to `Self::new_empty_with_profile(app, cfg.get_default_profile())`, querying the default monitor via `gdk::Display::default()` and sizing the window accordingly.
- **Profile-Specific Constructors:** Added `TilixWindow::new_empty_with_profile(app, profile)` and `TilixWindow::new_with_profile(app, profile)`.
- **Detached Window Sizing:** `detach_drag_to_new_window` retrieves the detached pane's profile via `pane.current_profile()` and sizes the new window using `TilixWindow::new_empty_with_profile(&app, &profile)`.

---

## 25. Terminal Font Zooming & Alt Drag-and-Drop Docking (Phase 13)

### 25.1 Dynamic Font Zooming Engine (`src/ui/terminal_pane.rs`)
- **Zoom Arithmetic & Precision:** Font scaling operates in incremental steps of $0.1$ (`ZOOM_STEP = 0.1`) bounded by $[0.2, 5.0]$ (`ZOOM_MIN` to `ZOOM_MAX`). Precision is preserved across repeated zoom increments and decrements via round-to-tenth arithmetic:
  $$\text{scale}_{\text{new}} = \text{clamp}\left(\frac{\text{round}((\text{scale}_{\text{current}} \pm 0.1) \times 10.0)}{10.0}, 0.2, 5.0\right)$$
  Reset returns scale to $1.0$ (`ZOOM_NORMAL`).
- **Internal Scale Tracking:** `TerminalPane` tracks logical scale in `Rc<Cell<f64>>` to prevent grid misalignment from underlying VTE minimum scale clamp ($0.25$).
- **Public API:** `font_scale(&self) -> f64`, `zoom_in(&self)`, `zoom_out(&self)`, `zoom_normal(&self)`.

### 25.2 Mouse Wheel Capture Interception
- **Capture Phase Controller:** A `gtk::EventControllerScroll` configured with `EventControllerScrollFlags::VERTICAL` is attached to the terminal widget using `PropagationPhase::Capture`.
- **Modifier Filtering:** Vertical scroll events are evaluated against `current_event_state()`:
  - When `CONTROL_MASK` is active and neither `SHIFT_MASK` nor `ALT_MASK` is active:
    - Upward scroll (`dy < 0.0`): triggers `zoom_in()` and returns `glib::Propagation::Stop`.
    - Downward scroll (`dy > 0.0`): triggers `zoom_out()` and returns `glib::Propagation::Stop`.
  - In all other circumstances (unmodified scrolling or when Shift/Alt are pressed), returns `glib::Propagation::Proceed`, permitting standard terminal buffer scrolling and text navigation.

### 25.3 Keybinding Catalog Expansion (`src/model/keybindings.rs`)
- **Expanded Catalog (27 Actions):** Added 3 dedicated zoom shortcut definitions under `ActionCategory::ViewAndSettings` (total 5 view actions):
  - `win.zoom-in` ("Zoom In", default accelerators: `<Primary>plus`, `<Primary>equal`, `<Primary>KP_Add`).
  - `win.zoom-out` ("Zoom Out", default accelerators: `<Primary>minus`, `<Primary>KP_Subtract`).
  - `win.zoom-normal` ("Normal Size", default accelerators: `<Primary>0`, `<Primary>KP_0`).
- **Action Dispatch Pipeline:** `TilixWindow` registers `zoom-in`, `zoom-out`, and `zoom-normal` `gio::SimpleAction`s in `setup_actions`, routing to `SessionView::zoom_in_active()`, `zoom_out_active()`, and `zoom_normal_active()`, which target the active session's currently focused `TerminalPane`.

### 25.4 Alt + Left-Click Terminal Drag Docking (`src/ui/dnd.rs`, `src/ui/terminal_pane.rs`)
- **Dual Drag Sources:** `TerminalPane::setup_drag_source` attaches independent drag sources to both:
  - Header bar (`TerminalPane.header`): `require_alt = false` (unrestricted left-click drag).
  - Terminal area (`TerminalPane.terminal`): `require_alt = true` (Alt-gated drag).
- **Sequence Denial & Selection Preservation (Issue #1335):**
  - In `drag_source.connect_begin`: if `require_alt` is true and `ALT_MASK` is absent, the gesture sequence is immediately rejected via `gesture.set_sequence_state(seq, gtk::EventSequenceState::Denied)` (or `gesture.set_state(EventSequenceState::Denied)`).
  - In `drag_source.connect_prepare`: if `require_alt` is true and `ALT_MASK` is absent, returns `None`.
  - This ensures standard terminal text selection, double-click word selection, and link clicking continue uninhibited unless the user explicitly holds `Alt` while initiating a drag.

---

## 26. OSC 52 and Desktop Clipboard Integration (Phase 14)

### 26.1 Pure Domain Streaming OSC 52 Parser (`src/pty/osc52.rs`)
- **State Machine Architecture:** Zero-dependency streaming parser implemented as a pure-domain state machine (`Osc52StreamParser`). States:
  - `Ground`: Regular terminal text and passthrough stream emission.
  - `Escape`: Prefix detection (`ESC` / `0x1B`). Consecutive `ESC` characters are safely preserved.
  - `OscHeader`: Validates the `52;` sequence prefix; non-52 OSC sequences (such as OSC 7 directory reporting or window title escapes) are transparently returned to passthrough.
  - `OscTargets`: Parses clipboard targets (`c`, `p`, `q`, `0`..`7`, compound targets like `cp`, or empty defaulting to `c`).
  - `OscPayload`: Gathers base64 encoded payload bytes up to safety bounds.
  - `EscapeInPayload`: Handles two-byte 7-bit String Terminator (`\x1b\\`), returning to payload collection if non-terminator bytes follow.
- **Terminator Detection:** Concurrently supports:
  - BEL (`\x07`)
  - 7-bit ST (`\x1b\\`)
  - 8-bit ST (`0x9C`)
- **Safety Limits & Memory Exhaustion Prevention:**
  - `MAX_OSC52_PAYLOAD_SIZE = 5 * 1024 * 1024` (5 MB). Payloads exceeding this safety threshold trigger an immediate safe reset to `Ground`, discarding excessive memory buffers and emitting subsequent stream bytes to passthrough.
- **Operations & Events:**
  - `Osc52Operation::Write(Vec<u8>)`: Decodes base64 payload via `glib::base64_decode`.
  - `Osc52Operation::Query`: Client requests clipboard contents (`?`).
  - `Osc52Operation::Clear`: Empty payload clearing the specified buffer.
- **Response Serializer:** `encode_osc52_response(target, data)` encodes binary payloads into standard OSC 52 responses (`\x1b]52;<target>;<base64>\x1b\\`) for queries.

### 26.2 In-Process PTY Proxy Interceptor (`src/pty/proxy.rs`)
- **Dual-PTY Architecture:** Uses `libc::openpty` to establish two PTY pairs:
  1. Inner PTY: `(inner_master, inner_slave)` — child process runs attached to `inner_slave` as standard input/output/error.
  2. Outer PTY: `(outer_master, outer_slave)` — VTE attaches to `outer_master` via `vte::Pty::foreign_sync`.
- **Bi-directional Worker Threads:**
  - `tilix-pty-input`: Reads user keystrokes from `outer_slave` and writes to `inner_master`. Synchronizes window sizing ioctls (`TIOCGWINSZ`/`TIOCSWINSZ`) during user interactions.
  - `tilix-pty-output`: Reads child process output from `inner_master`, passes stream chunks through `Osc52StreamParser`, writes non-OSC passthrough bytes to `outer_slave` for VTE rendering, and dispatches extracted OSC 52 events.
- **Thread-Safe Main Context Dispatch:** OSC 52 write and clear actions dispatch to the GTK main loop via `glib::idle_add_once`, updating desktop clipboards (`gdk::Display::default().clipboard()` and `primary_clipboard()`).
- **FD Lifecycle & EOF Propagation:** `PtyProxy` manages all 4 descriptors via `AtomicI32` slots. `ProxyPtyPair` and `PtyProxy` implement `Drop` to release remaining descriptors. The parent process calls `proxy.close_inner_slave()` immediately after `cmd.spawn()` so that `inner_master` receives EOF when the child process exits. `vte::Pty::foreign_sync` takes direct ownership of `outer_master` via `proxy.take_outer_master()`.
- **Window Size Synchronization (`TIOCSWINSZ` / `SIGWINCH`):** `TerminalPane` initializes the proxy's window size on spawn and listens to terminal dimension events (`connect_char_size_changed`, `connect_resize_window`, `notify::row-count`, `notify::column-count`), forwarding updates via `proxy.set_window_size()`. `tilix-pty-input` concurrently runs `sync_size_from_outer()` on user input forwarding.
- **Security Posture:** `osc52_allow_query` defaults to `false` in `Profile`, preventing untrusted terminal applications from querying sensitive user data unless explicitly enabled by the user.

### 26.3 Keybinding Catalog Expansion (`src/model/keybindings.rs`)
- **Expanded Catalog (32 Actions):** Added `ActionCategory::Clipboard` ("Clipboard & Edit") with 5 standard clipboard operations (Cut is omitted as terminal emulator buffers are read-only grid projections):
  - `win.copy` ("Copy", default accelerators: `<Primary><Shift>c`, `<Primary>Insert`).
  - `win.copy-html` ("Copy as HTML", default accelerators: none).
  - `win.paste` ("Paste", default accelerators: `<Primary><Shift>v`, `<Shift>Insert`).
  - `win.paste-primary` ("Paste Primary Selection", default accelerators: none).
  - `win.select-all` ("Select All", default accelerators: `<Primary><Shift>a`).
- **Collision Immunity:** All accelerators verified against existing categories with zero collisions across the complete 32-action catalog.

### 26.4 Context Menu, Copy-on-Select & UI Integration
- **Terminal Context Menu:** `TerminalPane` constructs a structured `gio::Menu` model with 4 functional sections (Clipboard, Selection, Splits, Preferences) and attaches it via `terminal.set_context_menu_model`.
- **Multi-Format Copy as HTML:** `copy_html()` creates a `GdkContentProvider` union containing both `text/html` (with ANSI formatting and colors) and `text/plain` fallbacks, ensuring compatibility across both rich-text editors and plain-text terminal pastes.
- **Frame-Synchronous Dimension Coordination & Mapped-Aware Spawn:** `TerminalPane` pre-configures VTE natural geometry via `terminal.set_size(default_cols, default_rows)` and defers child shell spawning until the widget is mapped with a valid size allocation (`terminal.is_mapped() && terminal.width() > 0 && terminal.column_count() > 0 && terminal.row_count() > 0`). In addition, `PtyProxy::set_window_size` deduplicates dimension updates with atomic caching to prevent redundant `TIOCSWINSZ` ioctls and spurious `SIGWINCH` signals, eliminating the prompt redraw race condition where `zsh` outputs `%` on startup (Tilix issue #1777).
- **Automatic Copy on Selection:** Connected via `terminal.connect_selection_changed`; when `profile.copy_on_select` is enabled and a selection exists, selected text is copied immediately using `vte4::Format::Text`.
- **Preferences Integration:**
  - Shortcuts tab includes `ActionCategory::Clipboard`.
  - Profile General tab includes `Automatically copy selection to clipboard` checkbutton.
  - Profile Compatibility tab includes `Allow terminal applications to set clipboard (OSC 52)` and `Allow terminal applications to read clipboard (OSC 52 query)` checkbuttons.

---

## 27. Compact Mode UI Density Optimization (Phase 15)

### 27.1 Motivation & Density Architecture
Power users working on compact screens, high-density laptops, or multi-split layouts frequently require maximum vertical terminal line count. Standard GNOME Libadwaita chrome allocates generous touch targets and relaxed padding:
- Standard `adw::HeaderBar`: 46px min-height (with 34px buttons).
- Standard `adw::TabBar`: 38px min-height.
- Standard `terminal-pane-header`: 36px height.
Combined, default chrome consumes **120px** of vertical height (~6 rows of terminal text on standard monospace grids).

Phase 15 introduces **Scheme A: Compact Mode**, compressing all chrome elements to yield **48px** (more than 2 full text rows) of vertical space back to the active terminal canvas without sacrificing window controls, tab visibility, or pane management tools, while eliminating the 3px Libadwaita spacing gap between top bars and terminal content.

### 27.2 Domain Model & Backwards Compatibility (`src/model/config.rs`)
- **Schema Extension:** `AppConfig` incorporates `#[serde(default)] pub compact_mode: bool`.
- **Default State:** `compact_mode: false` preserves standard GNOME HIG ergonomics out-of-the-box.
- **Serialization Safety:** Backwards-compatible deserialization guarantees legacy `config.json` payloads without the `compact_mode` key deserialize cleanly to `false` without data loss or schema faults.

### 27.3 Dynamic Geometry Engine (`src/ui/geometry.rs`)
- **Chrome Constants:**
  - `COMPACT_HEADER_BAR_HEIGHT = 28` (Δ = -18px from default 46px).
  - `COMPACT_TAB_BAR_HEIGHT = 22` (Δ = -16px from default 38px).
  - `COMPACT_PANE_HEADER_HEIGHT = 22` (Δ = -14px from default 36px).
- **Calculation Synchronization:** In `calculate_window_size_from_cell_size`, base chrome heights conditionally resolve to compact values when `app_config.compact_mode` is enabled, guaranteeing that initial window sizing for target column/row allocations matches the active density mode (yielding a 48px vertical saving for standard 80x24 profiles).
- **Hidden Chrome Invariance:** When toolbar, tabbar, and pane titles are hidden (`WindowStyle::HideToolbar`, `show_tab_bar: false`, `PaneTitleStyle::None`), compact and standard configurations evaluate to identical canvas dimensions.

### 27.4 CSS Density Styling (`src/ui/window.rs`)
The global stylesheet (`setup_css()`) is augmented with high-density `.compact` selectors:
- **Zero Top-Bar Gap:** Explicit `padding: 0` on `toolbarview > .top-bar` and `.collapse-spacing` eliminates the default Libadwaita 3px gap, allowing the tab bar to seat seamlessly against the terminal content.
- **HeaderBar & Window Title:** `toolbarview > .top-bar .collapse-spacing headerbar` and `headerbar > windowhandle > box` reset to `min-height: 28px` with 0 padding; `windowtitle` set to 20px min-height; header buttons set to `22px` with 16px icons.
- **Window Controls:** Minimize, maximize, and close buttons receive `20px` minimum bounds and `1px` padding with zero vertical margin, preventing clipping against the window edge.
- **TabBar, TabBox & Tabs:** High-specificity overrides for `toolbarview > .top-bar .collapse-spacing tabbar tabbox` compress `tabbox` to `22px` min-height with 0 padding; child `tab` compressed to `20px` min-height with `0 4px` padding and 18px close buttons; `.box` margin/box-shadow neutralized.
- **Terminal Pane Headers:** `.terminal-pane-header` compressed to `22px` min-height, padding tightened to `0 4px`, header buttons set to `18px`, and title labels scaled to `0.85em`.

### 27.5 Reactive Projection & Thread-Local Registry (`src/ui/window.rs`)
- **Weak Window Registry:** Windows are registered in `static WINDOW_INSTANCES: RefCell<Vec<glib::WeakRef<adw::ApplicationWindow>>>`.
- **Zero-Restart Live Updates:** `apply_compact_mode_to_all_windows(compact: bool)` dynamically adds or removes the `.compact` class across all active `WINDOW_INSTANCES`, `WINDOW_HEADER_BARS`, and `WINDOW_TAB_BARS` simultaneously, automatically pruning dead weak references.
- **Preferences UI Integration:** An interactive `adw::SwitchRow` in `Preferences -> Appearance -> Window` allows users to toggle Compact Mode instantly with immediate visual feedback across all open windows.

---

## 28. Dual Library Crate & Test Compilation Optimization (Phase 16)

### 28.1 Architectural Problem: The Parallel Compilation RAM Spike
Through Phases 1 through 15, Tilix expanded to over 16,600 lines of Rust across domain modeling, PTY orchestration, and GTK4/Libadwaita/VTE UI management. Integration test suites in `tests/` grew to 14 independent test binaries (`tests/test_phase2_domain.rs` through `tests/test_phase15_compact_mode.rs`), encompassing 194 automated test cases.

Because the project originated as a binary-only crate (`src/main.rs`), integration test binaries could not import the application as a library and instead relied on non-idiomatic `#[path = "../src/..."]` module inclusions. Consequently, Cargo compiled the entire 16.6k-line application codebase 15 independent times concurrently (1 binary + 14 test suites). During parallel release builds (`cargo test --release`, as mandated by distribution packaging checks in Arch Linux `PKGBUILD`), concurrent `rustc` instances generated an acute 15–30+ GB RAM spike, causing swap thrashing, system slowdowns, and Out-Of-Memory (`oom-killer`) terminations on CI and developer machines with <=16 GB RAM.

### 28.2 Dual Library + Binary Target Architecture
Phase 16 restructured Tilix into an idiomatic dual crate within the single package manifest (`Cargo.toml`):
- **Library Target (`[lib]`):**
  - Path: `src/lib.rs`
  - Name: `tilix`
  - Emits: `libtilix.rlib`
  - Exports: `pub mod app; pub mod model; pub mod pty; pub mod ui;`
  - Compiles the entire 16,621 lines of core application code exactly once. All internal references (`crate::model::...`, `crate::ui::...`) resolve seamlessly to the library root without changing any internal logic.
- **Binary Target (`[[bin]]`):**
  - Path: `src/main.rs`
  - Name: `tilix`
  - Emits: `target/{debug,release}/tilix`
  - Thin 6-line launcher that links against `tilix::app::TilixApplication` in <0.1s. Packaging scripts (`PKGBUILD`, `Makefile`, Flatpak manifests) remain 100% compatible with the emitted binary artifact.

```mermaid
flowchart TD
    subgraph Core_Library ["Core Library (libtilix.rlib)"]
        LibRoot["src/lib.rs"]
        AppMod["src/app.rs"]
        ModelMod["src/model/*"]
        PtyMod["src/pty/*"]
        UiMod["src/ui/*"]
        LibRoot --> AppMod
        LibRoot --> ModelMod
        LibRoot --> PtyMod
        LibRoot --> UiMod
    end

    subgraph Binary_Executable ["Binary Executable"]
        MainRoot["src/main.rs"]
        MainRoot -.->|Links| LibRoot
        BinArtifact["target/release/tilix"]
        MainRoot --> BinArtifact
    end

    subgraph Integration_Tests ["Integration Test Suites (tests/*.rs)"]
        T2["test_phase2_domain"]
        T3["test_phase3_domain"]
        T4["test_phase4_packaging_polish"]
        T5["test_phase5_window_style_wide_handle"]
        T6["test_phase6_pane_toolbar"]
        T7["test_phase7_keybindings"]
        T8["test_phase8_dnd"]
        T9["test_phase9_profile"]
        T10["test_phase10_profile_ui_parity"]
        T11["test_phase11_title_options"]
        T12["test_phase12_window_geometry"]
        T13["test_phase13_zoom_and_alt_drag"]
        T14["test_phase14_osc52_and_clipboard"]
        T15["test_phase15_compact_mode"]
        T2 & T3 & T4 & T5 & T6 & T7 & T8 & T9 & T10 & T11 & T12 & T13 & T14 & T15 -.->|use tilix::{...}| LibRoot
    end
```

### 28.3 Test Suite Modernization & Compilation Deduplication
All 14 integration test suites in `tests/` were migrated from `#[path = "..."] mod ...;` inclusions to idiomatic `use tilix::{app, model, pty, ui};` imports:
- **Phase 2–7 Domain & Config Suites:** Pure headless test suites import `use tilix::model;` or `use tilix::{app, model, pty, ui};`, compiling against `libtilix.rlib` in <0.05s on warm rebuilds.
- **Phase 8–15 UI & Scaffolding Suites:** Integration suites import `libtilix` modules cleanly, eliminating redundant widget tree monomorphization.

### 28.4 Optimization Outcomes & Benchmark Results
- **Compilation Multiplicity:** Reduced from 15x full-codebase compilations down to **1x** (`libtilix.rlib`).
- **Peak RAM Usage:** Lowered from **15–30+ GB** during parallel release testing to **<2.5 GB total**, completely eliminating `oom-killer` termination risks on resource-constrained systems.
- **Incremental Test Build Time:** Dropped by **80–90%** (from tens of seconds down to sub-second link times per test suite).
- **Zero Regressions:** 100% test pass rate preserved across all 194 unit and integration tests.





