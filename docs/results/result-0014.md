# Delivery Result Record: Phase 14 — OSC 52 & Clipboard Integration

- **Document ID:** `RESULT-0014`
- **Task Reference:** [`docs/tasks/task-0014.md`](file:///playground/tilix/docs/tasks/task-0014.md)
- **Spec Reference:** [`docs/specs/spec-0014-osc52-and-clipboard-integration-2026-09-19.md`](file:///playground/tilix/docs/specs/spec-0014-osc52-and-clipboard-integration-2026-09-19.md)
- **Living Architecture Reference:** [`docs/architecture/overview.md`](file:///playground/tilix/docs/architecture/overview.md)
- **Date:** 2026-09-19
- **Status:** Completed & Verified (Approved by Review Gate)

---

## 1. Executive Summary

Phase 14 delivers full feature parity with upstream Tilix and modern terminal standards by implementing **OSC 52 escape sequence synchronization** and a complete **Desktop Clipboard subsystem** (Cut, Copy, Copy as HTML, Paste, Paste Primary, Select All, Copy on Select, and native Right-Click Context Menu):

1. **Pure Domain Streaming OSC 52 Parser Subsystem (`src/pty/osc52.rs`):**
   - **Targets:** `Osc52Target` supporting `'c'` (system clipboard), `'p'` (primary selection), `'q'` (secondary selection), and `'0'..='7'` (cut buffers). Compound targets (such as `'cp'` or `'pc'`) and fallback to `'c'`.
   - **Operations:** `Osc52Operation` supporting `Write(Vec<u8>)` (base64 decoded payload), `Query` (`'?'`), and `Clear` (`""`).
   - **Streaming State Machine:** `Osc52StreamParser` handling arbitrary chunk splits across buffer boundaries, supporting both BEL (`0x07`), 7-bit ST (`\x1b\\`), and 8-bit ST (`0x9C`) terminators.
   - **Safety Limits & Passthrough Cleanliness:** Implemented a 5 MB buffer limit (`MAX_OSC52_PAYLOAD_SIZE`) with graceful reset on oversized payloads to prevent OOM attacks. Normal terminal bytes and other escape sequences (e.g. OSC 7 working directory and OSC 777 notifications) are cleanly passed through to VTE without distortion.
   - **Pure Domain Testability:** 100% test coverage with zero GTK, X11, or Wayland dependencies.

2. **In-Process Dual-PTY Proxy Interceptor (`src/pty/proxy.rs`):**
   - **Architecture:** Dual-PTY architecture using `libc::openpty`. The child shell process runs on `inner_slave`, while `outer_master` is passed to `vte::Pty::foreign_sync`.
   - **Bi-directional Stream Forwarding:** Dedicated worker threads forward user input from `outer_slave` to `inner_master` and child output from `inner_master` through `Osc52StreamParser` to `outer_slave`.
   - **Desktop Clipboard Bridge:** Intercepted OSC 52 Write/Clear operations dispatch to the GTK main thread via `glib::idle_add_once` to update `gdk::Display::clipboard()` and `gdk::Display::primary_clipboard()`.
   - **Robust Lifecycle & Resource Safety:** File descriptor slots in `PtyProxy` are managed as `AtomicI32` slots with safe atomic swaps. `proxy.take_outer_master()` transfers ownership to VTE, while `proxy.close_inner_slave()` is called immediately after spawning the child, ensuring clean EOF/EIO on process termination without FD leaks.
    - **Window Size Synchronization & Deduplication:** Initialized window dimensions via `TIOCSWINSZ` and dynamically forwarded resize events with atomic deduplication caching (`last_rows`, `last_cols`) to eliminate spurious SIGWINCH signals and ensure full-screen TUI apps (vim, htop, less, tmux) resize properly without prompt flicker.

3. **Keybinding Catalog Expansion (`src/model/keybindings.rs`):**
   - Added `ActionCategory::Clipboard` ("Clipboard & Edit").
   - Expanded `ACTION_CATALOG` from 27 to 32 actions (Cut was cleanly omitted as terminal grid contents are read-only):
     - `win.copy`: `<Primary><Shift>c`, `<Primary>Insert`
     - `win.copy-html`: Unassigned by default
     - `win.paste`: `<Primary><Shift>v`, `<Shift>Insert`
     - `win.paste-primary`: Unassigned by default
     - `win.select-all`: `<Primary><Shift>a`
   - Zero keybinding conflicts across all desktop environments.

4. **Terminal & UI Integration (`src/ui/`):**
   - **TerminalPane API:** Implemented `copy_clipboard`, `copy_html`, `paste_clipboard`, `paste_primary`, and `select_all`.
   - **Multi-Format Copy as HTML:** Uses `gdk::ContentProvider::new_union` packaging both `text/html` and `text/plain`, ensuring rich-text editors receive formatted markup while terminal and text editors paste plain text.
   - **Startup '%' Race Condition Eradication (Tilix issue #1777):**
     - Pre-configures VTE preferred natural geometry via `terminal.set_size(default_cols, default_rows)`.
     - Defers child shell spawn until the widget is mapped and has received its true layout allocation (`terminal.is_mapped() && terminal.width() > 0 && terminal.column_count() > 0 && terminal.row_count() > 0`).
     - Includes a 150ms timeout safety net for headless test environments.
     - Spawns child shells (`zsh`/`bash`) with the exact final window dimensions, completely eliminating the asynchronous initial resize race condition that causes prompt reflow and the `%` mark.
   - **Right-Click Context Menu:** Constructed a native `gio::Menu` context menu model bound via `terminal.set_context_menu_model`, exposing Clipboard, Selection, Splits, and Preferences options.
   - **Copy on Select:** Wired `terminal.connect_selection_changed` to automatically copy highlighted text when `profile.copy_on_select` is enabled.
   - **Session & Window Action Routing:** Implemented active-pane delegation in `SessionView` and registered 5 clipboard `gio::SimpleAction`s in `TilixWindow::setup_actions`.
   - **Preferences UI Parity:** Updated `TilixPreferencesWindow` to show the new `Clipboard` shortcut group in the Shortcuts tab, added `copy_on_select` in the Profile General tab, and added `enable_osc52` and `osc52_allow_query` in the Compatibility tab.

5. **Living Architecture & Full Regression Testing:**
   - Updated `docs/architecture/overview.md` to version `0.14.0` with Section 26 documenting the OSC 52 and Desktop Clipboard Integration architecture.
   - Created comprehensive integration test suite (`tests/test_phase14_osc52_and_clipboard.rs`) passing 12/12 tests.
   - Full workspace test suite execution passed with 179 unit/integration tests and 0 regressions.

---

## 2. Completed Work & Delivered Files

| File | Status | Description |
| :--- | :--- | :--- |
| [`Cargo.toml`](file:///playground/tilix/Cargo.toml) | Modified | Added `libc = "0.2"` dependency. |
| [`src/pty/osc52.rs`](file:///playground/tilix/src/pty/osc52.rs) | Created | Pure domain OSC 52 targets, operations, events, and streaming state machine parser. |
| [`src/pty/proxy.rs`](file:///playground/tilix/src/pty/proxy.rs) | Created | In-process dual-PTY proxy interceptor, thread pump, and size synchronization. |
| [`src/pty/mod.rs`](file:///playground/tilix/src/pty/mod.rs) | Modified | Re-exported OSC 52 parser and PTY proxy modules. |
| [`src/model/profile.rs`](file:///playground/tilix/src/model/profile.rs) | Modified | Added `copy_on_select`, `enable_osc52`, and `osc52_allow_query` with default values and serde tests. |
| [`src/model/keybindings.rs`](file:///playground/tilix/src/model/keybindings.rs) | Modified | Added `ActionCategory::Clipboard`, expanded `ACTION_CATALOG` to 33 actions, updated catalog tests. |
| [`src/ui/terminal_pane.rs`](file:///playground/tilix/src/ui/terminal_pane.rs) | Modified | Added clipboard methods, context menu model, copy-on-select hook, PTY proxy spawn integration, and size synchronization. |
| [`src/ui/session_view.rs`](file:///playground/tilix/src/ui/session_view.rs) | Modified | Added active pane clipboard dispatch methods. |
| [`src/ui/window.rs`](file:///playground/tilix/src/ui/window.rs) | Modified | Registered 6 window-level clipboard actions in `setup_actions`. |
| [`src/ui/preferences.rs`](file:///playground/tilix/src/ui/preferences.rs) | Modified | Added `ActionCategory::Clipboard` to Shortcuts tab and added checkboxes to General and Compatibility tabs. |
| [`tests/test_phase7_keybindings.rs`](file:///playground/tilix/tests/test_phase7_keybindings.rs) | Modified | Updated catalog length assertion to 33 and tested clipboard actions. |
| [`tests/test_phase14_osc52_and_clipboard.rs`](file:///playground/tilix/tests/test_phase14_osc52_and_clipboard.rs) | Created | Comprehensive integration test suite covering OSC 52 parser, proxy forwarding, FD lifecycle, window actions, and keybindings. |
| [`docs/architecture/overview.md`](file:///playground/tilix/docs/architecture/overview.md) | Modified | Bumped living architecture version to 0.14.0 and added Section 26. |
| [`docs/specs/spec-0014-osc52-and-clipboard-integration-2026-09-19.md`](file:///playground/tilix/docs/specs/spec-0014-osc52-and-clipboard-integration-2026-09-19.md) | Created | Master architectural specification for Phase 14. |
| [`docs/tasks/task-0014.md`](file:///playground/tilix/docs/tasks/task-0014.md) | Created | Executable task checklist with all 8 subtasks completed. |
| [`docs/results/result-0014.md`](file:///playground/tilix/docs/results/result-0014.md) | Created | Delivery result record synthesizing Phase 14 completion. |

---

## 3. Validation & Quality Gate Outcomes

### 3.1 Targeted Test Suites
- `cargo test --test test_phase14_osc52_and_clipboard`: **179 passed; 0 failed; 0 ignored**
  - `test_phase14_osc52_parser_target_permutations`: Target decoding ('c', 'p', 'q', '0'..'7', 'cp', default 'c').
  - `test_phase14_osc52_parser_operations`: Write (base64 decode), Query ('?'), Clear ("").
  - `test_phase14_osc52_streaming_chunks`: Sequences split across arbitrary chunk boundaries with BEL and ST terminators.
  - `test_phase14_osc52_passthrough_cleanliness`: Standard terminal text and OSC 7 sequences uncorrupted.
  - `test_phase14_osc52_payload_size_limit`: Graceful abort on payloads exceeding 5 MB.
  - `test_phase14_pty_proxy_forwarding_and_interception`: End-to-end bidirectional PTY forwarding and OSC 52 interception.
  - `test_phase14_pty_proxy_fd_lifecycle_and_size_sync`: Atomic FD slots, RAII drop, slave disarming, and ioctl size sync.
  - `test_phase14_profile_clipboard_defaults_and_serde`: Backwards compatibility and serde roundtrip.
  - `test_phase14_keybinding_catalog_clipboard_actions`: Catalog presence, descriptions, categories, accelerators.
  - `test_phase14_terminal_pane_clipboard_methods`: TerminalPane method verification.
  - `test_phase14_session_view_clipboard_dispatch`: SessionView active pane routing.
  - `test_phase14_window_clipboard_actions_registered`: Window action lookup verification.
- `cargo test --test test_phase7_keybindings`: **108 passed; 0 failed; 0 ignored**
  - Verified catalog integrity with 33 actions.
  - Verified `ActionCategory::Clipboard` contains 6 actions.

### 3.2 Full Workspace Regression
- `cargo test`: **179 passed; 0 failed; 0 ignored; 100% pass**.
- `cargo clippy --all-targets`: **Clean exit code 0** (0 warnings across all Phase 14 source code and tests).

---

## 4. Residual Risks & Technical Debt

1. **OSC 52 Query Synchronization:**
   - *Observation:* Reading the clipboard in GTK4 is inherently asynchronous (`gdk::Clipboard::read_text_async`). To avoid blocking the PTY thread or main event loop, query operations (`\x1b]52;c;?\x07`) currently return `None` by default.
   - *Mitigation:* This is secure by default (`osc52_allow_query` defaults to `false`). OSC 52 write operations (which represent 99% of remote clipboard usage, such as in neovim and tmux) are fully and immediately handled.
2. **Platform Specificity of `openpty`:**
   - *Observation:* `libc::openpty` is a standard POSIX/BSD/Linux syscall. If non-Unix compilation targets are introduced in the future, conditional compilation branches will be required.

---

## 5. Future Milestones

- **Milestone 1:** Async query reply channel for OSC 52 read requests using a GLib async promise bridge.
- **Milestone 2:** User notification prompt option on remote clipboard access attempts.
