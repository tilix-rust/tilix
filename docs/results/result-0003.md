# Delivery Result Record: Phase 3 — Wayland Quake, OSC CWD, DND & Interactive Preferences

- **Document ID:** `RESULT-0003`
- **Task Reference:** [`docs/tasks/task-0003.md`](file:///playground/tilix/docs/tasks/task-0003.md)
- **Spec Reference:** [`docs/specs/spec-0003-wayland-quake-osc-dnd-2026-09-15.md`](file:///playground/tilix/docs/specs/spec-0003-wayland-quake-osc-dnd-2026-09-15.md)
- **Living Architecture Reference:** [`docs/architecture/overview.md`](file:///playground/tilix/docs/architecture/overview.md)
- **Date:** 2026-09-16
- **Status:** Completed & Verified (Approved by Review Gate)

---

## 1. Executive Summary

Phase 3 of the Tilix Rust rewrite has been successfully completed and archived. We extended the multi-session terminal emulator with first-class desktop and Wayland integration:
- **OSC 7 Current Working Directory Inheritance:** Real-time directory tracking via `vte.connect_current_directory_uri_changed` and headless URI parsing (`parse_osc7_uri`), automatically spawning splits and new tabs in the active pane's working directory.
- **Wayland Quake Drop-Down Mode:** Top-docked, full-width `TilixQuakeWindow` controlled via single-instance D-Bus IPC (`GApplication` with `HANDLES_COMMAND_LINE`), allowing instant summoning or hiding from compositor shortcuts (`tilix --quake-toggle`) without legacy X11 global key grabs.
- **Desktop Notifications:** Automated alerts via `gio::Notification` when background builds/tasks complete or terminal bell events fire in unfocused panes or inactive tabs.
- **Wayland Drag-and-Drop (DND) & Tab Detaching:** Native tab detaching across windows via Libadwaita's `AdwTabView` create-window protocol, and visual pane reordering via GTK4 `DragSource` and `DropTarget` controllers coupled to pure domain `LayoutTree::swap_panes`.
- **Interactive Preferences Window:** Native `TilixPreferencesWindow` (`AdwPreferencesWindow`) supporting real-time profile adjustments (color schemes, cursor shapes, cursor blink modes, fonts) saved to `~/.config/tilix/config.json` (`AppConfig`) and reactively broadcast to all active terminals.

---

## 2. Completed Work & Delivered Files

| File | Status | Description |
| :--- | :--- | :--- |
| [`src/pty/shell.rs`](file:///playground/tilix/src/pty/shell.rs) | Modified | Implemented `parse_osc7_uri(uri: &str) -> Option<PathBuf>` handling `file://localhost/...`, `file:///...`, percent-encoded characters, and path normalization with 100% unit test coverage. |
| [`src/pty/mod.rs`](file:///playground/tilix/src/pty/mod.rs) | Modified | Re-exported `parse_osc7_uri`. |
| [`src/model/layout.rs`](file:///playground/tilix/src/model/layout.rs) | Modified | Implemented `swap_panes(&mut self, a: PaneId, b: PaneId) -> Result<(), LayoutError>` in pure domain model, preserving split ratios and tree structure. |
| [`src/model/config.rs`](file:///playground/tilix/src/model/config.rs) | Created | `AppConfig` struct with `Profile`, quake preferences, notification toggles, and JSON serde file persistence (`~/.config/tilix/config.json`). |
| [`src/model/mod.rs`](file:///playground/tilix/src/model/mod.rs) | Modified | Re-exported `config` module and `AppConfig`. |
| [`src/app.rs`](file:///playground/tilix/src/app.rs) | Modified | Configured `adw::Application` with `HANDLES_COMMAND_LINE`, implemented `parse_cli_args` (`CliAction`), D-Bus command-line routing across instances, and Quake window lifecycle. |
| [`src/main.rs`](file:///playground/tilix/src/main.rs) | Modified | Forwarded CLI arguments to `app.run_with_args(&args)`. |
| [`src/ui/quake.rs`](file:///playground/tilix/src/ui/quake.rs) | Created | `TilixQuakeWindow` wrapping `adw::ApplicationWindow` with `.quake-window` styling, top alignment, and visibility toggling. |
| [`src/ui/preferences.rs`](file:///playground/tilix/src/ui/preferences.rs) | Created | `TilixPreferencesWindow` (`AdwPreferencesWindow`) with Appearance page (Color Schemes, Cursor Shape, Cursor Blink, Font) and reactive change callbacks. |
| [`src/ui/notifications.rs`](file:///playground/tilix/src/ui/notifications.rs) | Created | `NotificationService` emitting `gio::Notification` alerts for bell events and child process completion. |
| [`src/ui/dnd.rs`](file:///playground/tilix/src/ui/dnd.rs) | Created | GTK4 `DragSource` and `DropTarget` helpers for dragging and dropping terminal panes. |
| [`src/ui/terminal_pane.rs`](file:///playground/tilix/src/ui/terminal_pane.rs) | Modified | Added CWD tracking (`current_directory`), initial directory spawning, bell/exit callbacks, and DND source/target controllers. |
| [`src/ui/session_view.rs`](file:///playground/tilix/src/ui/session_view.rs) | Modified | CWD forwarding on splits, notification emission for unfocused panes, and `swap_panes` projection updates without child shell restarts. |
| [`src/ui/window.rs`](file:///playground/tilix/src/ui/window.rs) | Modified | CWD inheritance on `new-tab`, tab detaching via `tab_view.connect_create_window`, cross-window session registry, and `win.preferences` action (`<Primary>comma`). |
| [`src/ui/mod.rs`](file:///playground/tilix/src/ui/mod.rs) | Modified | Re-exported `quake`, `preferences`, `notifications`, and `dnd`. |
| [`tests/test_phase3_domain.rs`](file:///playground/tilix/tests/test_phase3_domain.rs) | Created | Headless integration test suite verifying OSC 7 parsing variants, deep tree pane swapping, CLI argument parsing, and `AppConfig` persistence. |
| [`docs/architecture/overview.md`](file:///playground/tilix/docs/architecture/overview.md) | Updated | Living architecture document updated to v0.3.0 reflecting D-Bus IPC, Quake mode, DND pane swapping, tab detaching, and preferences. |
| [`docs/specs/spec-0003-wayland-quake-osc-dnd-2026-09-15.md`](file:///playground/tilix/docs/specs/spec-0003-wayland-quake-osc-dnd-2026-09-15.md) | Created | Master specification for Phase 3. |
| [`docs/tasks/task-0003.md`](file:///playground/tilix/docs/tasks/task-0003.md) | Created | Sequenced task execution checklist. |

---

## 3. Validation & Quality Gate Outcomes

### 3.1 Test Suite
All 121 tests pass cleanly with zero failures (`cargo test`):
- `unittests (src/main.rs)`: **41 passed** (LayoutTree, Shell/OSC 7, Session, Theming, Profile, CLI parsing, AppConfig)
- `tests/test_phase2_domain.rs`: **35 passed** (Ratio persistence, templates, sync routing)
- `tests/test_phase3_domain.rs`: **45 passed** (OSC 7 URI parsing variants, deep layout swapping, CLI arg parsing, AppConfig persistence)
- **Total:** **121 passed; 0 failed; 0 ignored; finished in 0.01s**

### 3.2 Linter & Type Safety
- `cargo check --all-targets`: **0 errors, 0 warnings**
- `cargo clippy --all-targets -- -D warnings`: **0 violations, 0 warnings**

### 3.3 Binaries Compilation
- `cargo build`: Debug binary produced at `target/debug/tilix`
- `cargo build --release`: Optimized release binary produced at `target/release/tilix`

---

## 4. Residual Risks & Tech Debt

1. **Wayland Top-Docking Ergonomics:**
   Under Wayland compositors (GNOME Mutter, KWin), application windows cannot force fixed screen coordinates without compositor window rules or protocols like `wlr-layer-shell`. The Quake window requests top-alignment and 100% monitor width, which compositors respect or configure via window rules.
2. **Dynamic Notification Title Updates:**
   In `SessionView`, the notification message title is captured at pane creation time. In Phase 4, this will be dynamically retrieved from the current pane title at event time.
3. **`WIDGET_TO_SESSION` Global State:**
   The cross-window session map uses thread-local storage with strong references. In Phase 4, transitioning to `Weak` references or a window destroy listener will ensure zero lingering references upon abrupt window termination.

---

## 5. Future Milestones (Phase 4: Packaging & Release)

- **Phase 4 (Packaging, Distribution & Final Polish):**
  - Arch Linux packaging: `PKGBUILD` for AUR / Arch repos.
  - Flatpak sandbox manifest: `org.gnome.Tilix.json` with desktop file and icon assets.
  - Desktop integration: `com.github.tilix_rust.desktop` and `metainfo.xml`.
  - Shell completion scripts (`bash`, `zsh`, `fish`).
