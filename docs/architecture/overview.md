# Tilix Rust Architecture Overview

**Status:** Living Architecture Document  
**Version:** 0.8.0 (Phase 8 Final Architecture)  
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
