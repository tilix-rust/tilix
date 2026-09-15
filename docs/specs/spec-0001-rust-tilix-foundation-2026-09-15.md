# Master Specification: Phase 1 — Tilix Rust Foundation

- **Document ID:** `SPEC-0001`
- **Date:** 2026-09-15
- **Status:** Approved / Decision Ready
- **Workflow Level:** Medium/Large (Phase 1 of Tilix Rewrite)
- **Target Location:** `/playground/tilix`

---

## 1. Context & Motivation

Tilix (written in D and GTK3) is unmaintained and incompatible with modern distribution packaging standards (Arch Linux dropped D compilers from official repos). This project rewrites Tilix in modern Rust using GTK4 (4.22.5), Libadwaita (1.9.4), and VTE4 (0.84.1 / crate v0.10.0).

Phase 1 lays the foundational architecture:
- Headless, fully unit-tested split-tree layout model.
- GTK4 reactive paned UI projection.
- `TerminalPane` composite widget with VTE4 child shell spawning and lifecycle management.
- Libadwaita window with essential keyboard shortcuts (`Ctrl+Shift+R`, `Ctrl+Shift+D`, `Ctrl+Shift+W`, `Alt+Arrows`).

---

## 2. Goals & Non-Goals

### Goals
1. **Pure Headless Layout Tree:** Implement `LayoutTree` and `LayoutNode` in `src/model/layout.rs` with 100% unit-test coverage for splitting, closing, ratio adjustments, balancing, and directional neighbor resolution without GTK or X11/Wayland initialized.
2. **PTY Shell Spawning:** Implement `TerminalPane` wrapping `vte4::Terminal`, spawning `$SHELL` asynchronously, handling process exit, and providing title synchronization.
3. **Reactive Paned Projection:** Implement `SessionView` projecting `LayoutTree` into nested `gtk::Paned` containers, reusing running `TerminalPane` instances without resetting shell processes.
4. **Desktop Application Integration:** Implement `AdwApplication` and `AdwApplicationWindow` providing a native GNOME header bar, keyboard accelerators, and split/close buttons.
5. **Zero-Panic Stability:** Robust error handling using Rust `Result` types for all layout operations and PTY spawning.

### Non-Goals (Explicitly Deferred to Subsequent Phases)
- Multi-session tab bar / `AdwTabView` (deferred to Phase 2).
- Layout profile persistence and GSettings schema configuration (deferred to Phase 2).
- Key broadcast execution across sync groups (foundation types in Phase 1; broadcast transmission in Phase 2).
- Custom color palette / font preference dialog (deferred to Phase 2).
- Quake / drop-down terminal mode (deferred to Phase 3).
- Session drag-and-drop between windows (deferred to Phase 3).

---

## 3. Functional Requirements

### FR-1: PTY Child Shell Spawning
- The system must read the `$SHELL` environment variable (falling back to `/bin/sh`).
- Spawning must occur asynchronously via `vte4::Terminal::spawn_async` with `PtyFlags::DEFAULT`.
- When the child process terminates, the terminal pane must trigger automatic pane closure.
- When the child process sets window title escape codes, the pane header label must update.

### FR-2: Horizontal Splitting (Side-by-Side)
- Triggered by shortcut `Ctrl+Shift+R` or header bar "Split Right" button.
- The active `Leaf(target_id)` in `LayoutTree` is replaced by `Split { orientation: Horizontal, ratio: 0.5, first: Leaf(target_id), second: Leaf(new_pane_id) }`.
- A new `TerminalPane` is spawned and attached as the right child.
- Focus is automatically transferred to the newly spawned pane.

### FR-3: Vertical Splitting (Top-and-Bottom)
- Triggered by shortcut `Ctrl+Shift+D` or header bar "Split Down" button.
- The active `Leaf(target_id)` in `LayoutTree` is replaced by `Split { orientation: Vertical, ratio: 0.5, first: Leaf(target_id), second: Leaf(new_pane_id) }`.
- A new `TerminalPane` is spawned and attached as the bottom child.
- Focus is automatically transferred to the newly spawned pane.

### FR-4: Pane Closure
- Triggered by shortcut `Ctrl+Shift+W`, header close button, or shell process exit (`exit` / `Ctrl+D`).
- The active `Leaf(target_id)` is removed from `LayoutTree`.
- The parent `Split` node is collapsed, replacing the split with the sibling branch.
- Focus is shifted to an adjacent sibling pane.
- If the closed pane was the last remaining pane in the window, the window closes cleanly.

### FR-5: Directional Focus Navigation
- Triggered by `Alt+Up`, `Alt+Down`, `Alt+Left`, `Alt+Right`.
- The model computes the visible neighbor using 2D normalized bounding box geometry.
- Focus is immediately granted to the neighbor's `vte4::Terminal`.

### FR-6: Visual Focus Indicator
- The currently focused `TerminalPane` must visually differentiate itself from unfocused panes by applying the `.active-pane` CSS style class (accent border).

---

## 4. Non-Functional Requirements

### NFR-1: Headless Unit Testability
- All operations in `src/model/layout.rs` must execute and pass in headless CI without a display server (`DISPLAY` or `WAYLAND_DISPLAY` not required).

### NFR-2: Memory Safety & Resource Cleanup
- Closing a pane must terminate the PTY, drop the `vte4::Terminal`, and remove the `TerminalPane` from the widget map to prevent memory or file descriptor leaks.

### NFR-3: GNOME Human Interface Guidelines (HIG)
- Application must use Libadwaita widgets (`AdwApplication`, `AdwApplicationWindow`, `AdwHeaderBar`), standard icons (`object-flip-horizontal-symbolic`, `object-flip-vertical-symbolic`, `window-close-symbolic`), and honor system dark/light styling.

### NFR-4: Performance
- Tree transformations and paned projection rebuilds must complete in under 16ms (60 FPS responsiveness).

---

## 5. Detailed Data Structures & API Design

### 5.1 `src/model/layout.rs`
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PaneId(pub u64);

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

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum LayoutError {
    #[error("Pane ID {0:?} not found in layout tree")]
    PaneNotFound(PaneId),
    #[error("Cannot split: tree invariant violated")]
    InvalidSplit,
    #[error("Cannot close last remaining pane")]
    CannotCloseLastPane,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LayoutNode {
    Leaf(PaneId),
    Split {
        orientation: SplitOrientation,
        ratio: f64,
        first: Box<LayoutNode>,
        second: Box<LayoutNode>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutTree {
    root: LayoutNode,
}

impl LayoutTree {
    pub fn new(initial_pane: PaneId) -> Self;
    pub fn root(&self) -> &LayoutNode;
    pub fn panes(&self) -> Vec<PaneId>;
    pub fn contains(&self, id: PaneId) -> bool;
    pub fn split(&mut self, target: PaneId, orientation: SplitOrientation, new_pane: PaneId) -> Result<(), LayoutError>;
    pub fn close(&mut self, target: PaneId) -> Result<Option<PaneId>, LayoutError>;
    pub fn balance(&mut self);
    pub fn find_adjacent(&self, current: PaneId, direction: Direction) -> Option<PaneId>;
}
```

### 5.2 `src/ui/terminal_pane.rs`
```rust
pub struct TerminalPane {
    container: gtk::Box,
    header: gtk::Box,
    title_label: gtk::Label,
    terminal: vte4::Terminal,
    pane_id: PaneId,
}

impl TerminalPane {
    pub fn new(pane_id: PaneId) -> Self;
    pub fn pane_id(&self) -> PaneId;
    pub fn widget(&self) -> &gtk::Widget;
    pub fn terminal(&self) -> &vte4::Terminal;
    pub fn set_active(&self, active: bool);
    pub fn set_title(&self, title: &str);
    pub fn grab_focus(&self);
}
```

### 5.3 `src/ui/session_view.rs`
```rust
pub struct SessionView {
    container: gtk::Box,
    panes: HashMap<PaneId, TerminalPane>,
    layout: LayoutTree,
    active_pane: Option<PaneId>,
    next_pane_id: u64,
}

impl SessionView {
    pub fn new() -> Self;
    pub fn split_active(&mut self, orientation: SplitOrientation);
    pub fn close_active(&mut self);
    pub fn focus_adjacent(&mut self, direction: Direction);
    pub fn active_pane_id(&self) -> Option<PaneId>;
    fn rebuild_projection(&mut self);
}
```

---

## 6. Decision Gating Log

| Topic | Resolution | Source | Reason |
| --- | --- | --- | --- |
| **GUI Framework** | Rust + GTK4 + Libadwaita + VTE4 | Fact | Direct replacement for GTK3 D stack; available natively on Arch Linux. |
| **Model Separation** | Pure headless domain tree in `src/model/layout.rs` | Decision | Allows 100% headless testing without display server or GTK loop. |
| **Reparenting Strategy** | Cache `TerminalPane` in `HashMap` and reparent into paned containers | Decision | Preserves live PTY process and scrollback on layout restructuring. |
| **Directional Navigation** | 2D geometric bounding box interval overlap | Decision | Guarantees deterministic directional navigation across asymmetrical splits. |
| **Accelerators** | Registered via `gio::SimpleAction` on `AdwApplicationWindow` | Decision | Clean separation of global shortcuts from terminal key capture. |

---

## 7. Validation & Test-First Strategy

### 7.1 Unit Test Coverage Requirements (`src/model/layout.rs`)
- `test_new_layout_has_single_leaf`
- `test_split_horizontal_and_vertical`
- `test_split_nested_deep_tree`
- `test_close_leaf_promotes_sibling`
- `test_close_nested_branch_promotes_subtree`
- `test_close_last_pane_returns_none`
- `test_directional_navigation_2x2_grid`
- `test_directional_navigation_asymmetrical`
- `test_balance_split_ratios`

### 7.2 Validation Commands
```bash
cargo check --all-targets
cargo test --lib model::layout
cargo test
cargo build
```
