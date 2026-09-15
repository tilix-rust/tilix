# Tilix Rust Architecture Overview

**Status:** Living Architecture Document  
**Version:** 0.2.0 (Phase 2 Architecture)  
**Date:** 2026-09-15  

---

## 1. System Overview & Mission

Tilix is an advanced GTK-based tiling terminal emulator originally authored by Gerald Nunn in the D programming language using GTK3. While celebrated for its intuitive split-terminal workflow, native GNOME feel, and input synchronization, original Tilix suffered from severe architectural debt:
1. **Coupled Widget-as-State Model:** The GTK widget hierarchy *was* the session state. Tree manipulation (splitting, closing, rebalancing) manipulated live GTK3 containers directly, making headless unit testing impossible, serialization brittle, and layout transitions prone to widget layout bugs.
2. **Ecosystem Stagnation:** D language tooling and libraries in desktop distributions have waned (e.g., D compilers and packages were dropped from Arch Linux official repositories).
3. **Legacy Display Assumptions:** GTK3 abstractions lacked first-class Wayland ergonomics, modern Libadwaita adaptive design patterns, and GTK4 event controllers.

### Rust Tilix Mission
Rewrite Tilix in modern Rust using **GTK4**, **Libadwaita**, and **VTE4**, adhering to the following guiding tenets:
- **Headless Domain Core:** The terminal layout tree, session state, profiles, and templates are pure, immutable-friendly algebraic data structures with zero dependencies on GTK, Wayland, or X11. They are 100% unit-testable in CI without a display server.
- **Unidirectional Reactive Projection:** The UI layer is a projection of the headless domain model. When the tree changes, the UI reconciles its `gtk::Paned` containers while preserving running `vte4::Terminal` instances.
- **Rock-Solid Process & PTY Lifecycle:** Terminal child processes are spawned asynchronously with strict signal and exit handling to prevent zombie processes and broken pipes.
- **Native GNOME HIG:** Full integration with Libadwaita styling, accent colors, header bars, adaptive tab bar with autohide, and responsive Wayland gestures.
- **Zero-Feedback Synchronized Input:** Seamless input broadcast across terminal panes using `vte.connect_commit` and `vte.feed_child`.

---

## 2. High-Level Architectural Block Diagram

```mermaid
flowchart TD
    subgraph UI_Layer ["UI Layer (GTK4 + Libadwaita)"]
        Win["AdwApplicationWindow<br/>(HeaderBar, TabBar, Accelerators)"]
        TabBar["AdwTabBar<br/>(autohide = true)"]
        TabView["AdwTabView<br/>(Multi-Session Coordinator)"]
        Page1["AdwTabPage (Tab 1)"]
        Page2["AdwTabPage (Tab 2)"]
        SV1["SessionView 1"]
        SV2["SessionView 2"]
        P1["gtk::Paned (SplitId 1)"]
        TP1["TerminalPane 1 (Sync: ON)"]
        TP2["TerminalPane 2 (Sync: ON)"]
        TP3["TerminalPane 3 (Sync: OVERRIDE)"]
    end

    subgraph Domain_Model ["Headless Domain Model (Pure Rust)"]
        SM1["SessionModel 1<br/>(sync_input: true)"]
        LT1["LayoutTree 1<br/>(SplitId 1, ratio: 0.62)"]
        Prof["Profile / ColorScheme<br/>(16 ANSI Palettes, JSON)"]
        Tmpl["SessionLayoutTemplate<br/>(JSON Export/Import)"]
    end

    subgraph Sync_Bus ["Synchronized Input Broadcast Bus"]
        CommitHook["Active Terminal commit(text)"]
        FeedEngine["feed_child(bytes) Broadcast"]
    end

    Win --> TabBar
    Win --> TabView
    TabBar -.->|Binds to| TabView
    TabView --> Page1
    TabView --> Page2
    Page1 --> SV1
    Page2 --> SV2
    SV1 --> P1
    P1 --> TP1
    P1 --> P2
    P2 --> TP2
    P2 --> TP3

    SV1 <-->|Syncs with| SM1
    SM1 --> LT1
    SV1 -.->|Applies| Prof
    SM1 -.->|Serialized via| Tmpl

    TP1 -->|1. User types| CommitHook
    CommitHook -->|2. If sync active| FeedEngine
    FeedEngine -->|3. Replicates to| TP2
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
    /// Side-by-side panes (left and right), separated by a vertical divider.
    Horizontal,
    /// Stacked panes (top and bottom), separated by a horizontal divider.
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
        ratio: f64, // Clamped between 0.05 and 0.95 (default 0.5)
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
3. **Splitting:**
   - `split(target: PaneId, orientation: SplitOrientation, new_pane: PaneId) -> Result<(), LayoutError>`
   - Replaces `Leaf(target)` with `Split { id, orientation, ratio: 0.5, first: Box::new(Leaf(target)), second: Box::new(Leaf(new_pane)) }`.
4. **Closing & Collapsing:**
   - `close(target: PaneId) -> Result<Option<PaneId>, LayoutError>`
   - Finds the parent split of `Leaf(target)`. Replaces the parent split with the target's sibling branch.
   - If the target is the root and only leaf, sets `self.root = None` and returns `Ok(None)` indicating the tree has emptied.
   - Returns `Ok(Some(sibling_id))` indicating the next pane that should receive focus.
5. **Balancing:**
   - `balance()`: Recursively resets split ratios. In binary mode, sets every ratio to `0.5`. In proportional mode, weights ratios by the leaf count of each subtree:
     $$\text{ratio} = \frac{\text{count}(\text{first})}{\text{count}(\text{first}) + \text{count}(\text{second})}$$
6. **Dynamic Split Ratio Updates:**
   - `set_split_ratio(split_id: SplitId, ratio: f64) -> bool`: Locates the split node and updates its ratio (clamped `0.05..0.95`).
7. **Geometric Directional Navigation:**
   - Without GTK, the tree calculates the normalized 2D bounding boxes `[0.0, 1.0] x [0.0, 1.0]` of all leaves.
   - For a given focused `PaneId` and `Direction`, it computes ray-cast intervals to find the closest visible adjacent pane.

---

## 4. UI Projection & Event Flow

### 4.1 Projection Strategy
The UI layer maintains a pool of active `TerminalPane` widgets keyed by `PaneId`:
```rust
pub struct SessionView {
    container: gtk::Box,
    panes: HashMap<PaneId, TerminalPane>,
    model: SessionModel,
}
```
When `LayoutTree` changes (split, close, rebalance):
1. The tree is traversed recursively.
2. For each `LayoutNode::Leaf(pane_id)`, the existing `TerminalPane` is retrieved from `panes`.
3. For each `LayoutNode::Split { id, orientation, ratio, .. }`, a `gtk::Paned` is allocated with the designated orientation and divider position.
4. The children are attached using GTK4's `set_start_child(Some(&child1))` and `set_end_child(Some(&child2))`.
5. Because `TerminalPane` instances are kept in `HashMap<PaneId, TerminalPane>`, reparenting them into a new `gtk::Paned` tree preserves their internal `vte4::Terminal` widget, PTY connection, running child process, and scrollback buffer without interruption.

### 4.2 Split Ratio Synchronization
- When the user drags a `gtk::Paned` divider bar, the GTK `notify::position` signal fires.
- `SessionView` calculates the new fraction relative to the allocated pixel width/height:
  $$\text{ratio} = \left(\frac{\text{paned.position}()}{\text{paned.width()} \text{ or } \text{paned.height()}}\right).\text{clamp}(0.05, 0.95)$$
- The updated ratio is stored back into `LayoutTree::set_split_ratio(split_id, ratio)` so that subsequent layout modifications retain the user's manual adjustments.

### 4.3 Focus Management
- Each `TerminalPane` listens for focus changes via `gtk::EventControllerFocus`.
- When focus enters a terminal:
  1. The active `PaneId` in `SessionModel` is updated.
  2. `SessionView::set_active_pane` acts as the sole controller for applying `.active-pane` CSS styling.

---

## 5. PTY & VTE Integration

### 5.1 TerminalPane Composition
A `TerminalPane` is a composite widget (`gtk::Box` vertical):
- **Header Bar:** Lightweight header containing:
  - Terminal title label (dynamically bound to shell/process title).
  - Sync override button (`input-keyboard-symbolic`).
  - Quick action buttons: Split Right, Split Down, Close.
- **Terminal Area:** `vte4::Terminal` configured for high-performance rendering.

### 5.2 Shell Spawning
Child shells are spawned asynchronously via `vte4::Terminal::spawn_async`:
- **Shell Resolution:** Checks `$SHELL` environment variable; falls back to `/bin/sh`.
- **Arguments:** Default `[shell.as_str()]`.
- **Environment:** Inherits parent process environment with standard terminal variables (`TERM=xterm-256color`, `COLORTERM=truecolor`).
- **Working Directory:** Preserved from parent or configured working directory.
- **PTY Flags:** `vte4::PtyFlags::DEFAULT`.

### 5.3 Signal Handling & Child Exit
- `terminal.connect_child_exited`: When the child shell exits (`exit` or `Ctrl+D`), dispatches pane closure to `SessionView`.
- `terminal.connect_window_title_changed`: Reads `term.window_title()` and updates header title and parent tab title.

---

## 6. Multi-Session Tab Management (`AdwTabView` & `AdwTabBar`)

In Phase 2, `TilixWindow` manages multiple independent sessions using Libadwaita's `AdwTabView` and `AdwTabBar`:
1. **Separation of Concerns:** Each tab page (`AdwTabPage`) wraps a distinct `SessionView`. State mutations, split operations, and shell child processes within one tab are completely isolated from other tabs.
2. **HIG Autohide:** The `AdwTabBar` sits in `ToolbarView::add_top_bar` with `autohide` set to `true`. When only a single tab is open, the tab bar is concealed to maximize vertical terminal space. Opening a second tab (`Ctrl+Shift+T`) automatically reveals the tab bar.
3. **Dynamic Title Propagation:** As terminal processes execute and update their OSC window titles, `TerminalPane` fires a title notification that propagates through `SessionView` to `AdwTabPage::set_title()`.
4. **Coordinated Tab & Pane Closure:** The `<Primary><Shift>w` shortcut contextually evaluates session depth: if multiple panes exist in the active tab, it closes the focused pane; if only a single pane remains, it closes the tab page. Closing the final tab closes the window.

---

## 7. Synchronized Input Broadcast Engine

Tilix provides real-time input synchronization to execute simultaneous administration commands across multiple terminal panes:
1. **Interception via `connect_commit`:** When the user types into the focused `vte4::Terminal`, VTE's internal IM context and key event handlers emit the `commit` signal with the processed text string (including Unicode characters and control characters like Enter and Tab).
2. **Cycle-Free Replication via `feed_child`:**
   - The active pane's `commit` signal triggers `SessionView::broadcast_input(sender_pane_id, text)`.
   - If session-wide sync is enabled and the sender pane has not opted out, `SessionView` iterates all sibling `TerminalPane` instances in the same session/sync group.
   - For each eligible destination pane, it calls `destination_pane.feed_child(text.as_bytes())`.
   - Because `feed_child` writes directly to the destination child PTY and does not emit the widget's `commit` signal, input replication is completely free from infinite loops or recursive echoes.
3. **Dual-Tier Control:**
   - **Session Sync Toggle:** Controlled via `Ctrl+Shift+I` and a header bar toggle button with icon `network-transmit-receive-symbolic`.
   - **Per-Pane Sync Override:** Each pane header contains an independent sync button (`input-keyboard-symbolic`) allowing users to safely isolate sensitive terminals (e.g. password prompts or single-node tasks) from the broadcast stream.

---

## 8. Profiles, Theming & Palette Subsystem

1. **Headless Domain Model:** Color schemes and terminal profiles are defined in `src/model/theme.rs` and `src/model/profile.rs` without dependencies on GTK or display servers.
2. **Tilix & GNOME JSON Palettes:** Supports standard Tilix JSON themes containing:
   - `foreground-color`, `background-color`, `cursor-background-color`
   - `palette`: Exactly 16 hex color strings representing ANSI standard and bright colors.
3. **Native VTE Application:** The UI layer converts parsed `RgbColor` floats to `gtk::gdk::RGBA` and invokes:
   - `terminal.set_colors(Some(&fg), Some(&bg), &palette_refs)`
   - `terminal.set_color_cursor(cursor_ref)`

---

## 9. Split Ratio Persistence & Session Templates

1. **Addressable Split Nodes:** `LayoutNode::Split` includes a unique `SplitId`.
2. **Divider Tracking:** During reactive UI projection, `SessionView` connects to `gtk::Paned`'s `notify::position` signal. Manual handle drags compute the relative ratio and write it back to `LayoutTree::set_split_ratio(split_id, ratio)`. Subsequent tree transformations preserve the user's customized proportions.
3. **Session Layout Templates:** `SessionLayoutTemplate` serializes session structures to JSON via `serde`. On import, `instantiate_session` sequentially renumbers all `PaneId`s and `SplitId`s, preventing ID clashes when importing templates into existing windows.

---

## 10. Wayland, Desktop Integration & GNOME HIG

- **Native Wayland First:** Uses standard GTK4 and Libadwaita windowing. Fractional scaling, touch gestures, and Wayland seat events are handled natively by GTK4.
- **Libadwaita Styling:** Uses `adw::Application` and `adw::ApplicationWindow` with standard GNOME 4x design patterns:
  - Header bar with title, window controls, sync button, and split buttons.
  - Adaptive styling responsive to GNOME system-wide dark/light mode preference (`AdwStyleManager`).
  - Standard GNOME keyboard shortcuts (`Ctrl+Shift+T`, `Ctrl+Shift+W`, `Ctrl+Shift+R`, `Ctrl+Shift+D`, `Ctrl+Shift+I`, `Ctrl+Shift+B`, `Ctrl+PageDown`, `Ctrl+PageUp`, `Alt+1..9`, `Alt+Arrows`).
