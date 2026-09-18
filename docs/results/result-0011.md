# Delivery Result Record: Phase 11 — Default Session Name & Application Title Settings

- **Document ID:** `RESULT-0011`
- **Task Reference:** [`docs/tasks/task-0011.md`](file:///playground/tilix/docs/tasks/task-0011.md)
- **Spec Reference:** [`docs/specs/spec-0011-session-and-application-title-options-2026-09-18.md`](file:///playground/tilix/docs/specs/spec-0011-session-and-application-title-options-2026-09-18.md)
- **Living Architecture Reference:** [`docs/architecture/overview.md`](file:///playground/tilix/docs/architecture/overview.md)
- **Date:** 2026-09-18
- **Status:** Completed & Verified (Approved by Review Gate)

---

## 1. Executive Summary

Phase 11 introduces comprehensive title customization options to Tilix Rust, implementing **Default session name** and **Application title** preferences matching upstream Tilix capabilities:

1. **Domain Model Extensions (`src/model/config.rs`):**
   - Extended `AppConfig` with `default_session_name` (defaulting to `"${title}"`) and `app_title` (defaulting to `"${appName}: ${sessionName}"`).
   - Maintained 100% serde forward and backward compatibility with `#[serde(default = "...")]`, ensuring legacy JSON configurations deserialize transparently.

2. **Hierarchical Token Evaluation Engine (`src/model/title.rs` & `src/model/mod.rs`):**
   - Pure, headless domain token evaluation engine supporting the entire upstream Tilix variable catalog across four hierarchical scopes:
     - **Terminal Scope (12 tokens + profile):** `${title}`, `${iconTitle}`, `${id}`, `${directory}`, `${hostname}`, `${username}`, `${columns}`, `${rows}`, `${process}`, `${status.readonly}`, `${status.silence}`, `${status.input-sync}`, and `${profile}`.
     - **Session Scope (3 tokens):** `${activeTerminalTitle}`, `${terminalCount}`, and `${terminalNumber}`.
     - **Window Scope (4 tokens):** `${appName}`, `${sessionName}`, `${sessionNumber}`, and `${sessionCount}`.
     - **Badge Scope:** Terminal tokens plus session and environment identifiers.
   - Robust parsing algorithms providing graceful fallbacks for absent context, handling unclosed or unknown tokens without panic, and supporting token fallbacks (e.g. `${token:fallback}`).
   - Preserved full backward compatibility for existing callers (`expand_tokens`, `expand_title_format`, and `expand_badge_format`) in `src/model/profile.rs`.

3. **Reusable Scoped Token Menu Popover (`src/ui/preferences.rs`):**
   - Implemented `create_scoped_token_menu_button(target_entry: &gtk::Entry, scope: TitleEditScope)` matching upstream Tilix `titleeditor.d` architecture.
   - Provides clean categorized sections: Terminal variables, Session variables (in Session/Window scopes), Window variables (in Window scope), and a Help section linking to official Tilix documentation (`https://gnunn1.github.io/tilix-web/manual/title/`).
   - Integrated cursor-position text insertion (`target_entry.insert_text(tok, &mut pos)`) with focus retention, allowing users to compose complex title templates without wiping surrounding text.
   - Upgraded profile terminal title and badge token buttons to use the scoped popover helper.

4. **Appearance Preferences UI Integration (`src/ui/preferences.rs`):**
   - Added **Default session name** (`TitleEditScope::Session`) and **Application title** (`TitleEditScope::Window`) controls under the Appearance preferences page.
   - Wired live reactive signal connections to auto-save configuration and trigger instantaneous cross-window and tab title recalculations.

5. **Live Dynamic Title Synchronization (`src/ui/session_view.rs` & `src/ui/window.rs`):**
   - `SessionView` observes terminal pane title changes, directory changes, process changes, and active pane switches, resolving `default_session_name` into dynamic session tab titles.
   - `TilixWindow` registers an updater closure in `WINDOW_TITLE_UPDATERS` and updates both the window title (`window.set_title`) and header bar title widget (`title_widget.set_title`) dynamically on active tab switches, session additions/removals, terminal title events, and preference changes.
   - Guarded against re-entrant `RefCell` borrowing panics with safe `try_borrow` semantics during tab and split lifecycles.

---

## 2. Completed Work & Delivered Files

| File | Status | Description |
| :--- | :--- | :--- |
| [`src/model/config.rs`](file:///playground/tilix/src/model/config.rs) | Modified | Added `default_session_name` and `app_title` fields to `AppConfig` with default providers and serde compatibility unit tests. |
| [`src/model/title.rs`](file:///playground/tilix/src/model/title.rs) | Created | Pure domain token evaluation engine, `TitleEditScope` enum, `TokenContext` struct, token catalog definitions, and scoped expansion functions. |
| [`src/model/mod.rs`](file:///playground/tilix/src/model/mod.rs) | Modified | Re-exported `title` module items (`TitleEditScope`, `TokenContext`, `expand_title_tokens`, `expand_title_tokens_scoped`). |
| [`src/model/profile.rs`](file:///playground/tilix/src/model/profile.rs) | Modified | Re-routed legacy token formatting functions through the unified token expansion engine with 100% backward compatibility. |
| [`src/ui/preferences.rs`](file:///playground/tilix/src/ui/preferences.rs) | Modified | Implemented `create_scoped_token_menu_button` with cursor insertion; added "Default session name" and "Application title" rows to Appearance page with live sync. |
| [`src/ui/session_view.rs`](file:///playground/tilix/src/ui/session_view.rs) | Modified | Added `build_token_context`, dynamic session title evaluation, and pane title change propagation. |
| [`src/ui/window.rs`](file:///playground/tilix/src/ui/window.rs) | Modified | Added `compute_and_apply_window_title`, `WINDOW_TITLE_UPDATERS` registry, and `apply_title_settings_to_all_windows()` broadcast helper. |
| [`docs/architecture/overview.md`](file:///playground/tilix/docs/architecture/overview.md) | Modified | Bumped living architecture version to 0.11.0 and added Section 23 documenting the Title Options & Token Subsystem. |
| [`docs/specs/spec-0011-session-and-application-title-options-2026-09-18.md`](file:///playground/tilix/docs/specs/spec-0011-session-and-application-title-options-2026-09-18.md) | Created | Master architectural specification for Phase 11. |
| [`docs/tasks/task-0011.md`](file:///playground/tilix/docs/tasks/task-0011.md) | Created | Executable task checklist with all 8 subtasks completed. |
| [`tests/test_phase11_title_options.rs`](file:///playground/tilix/tests/test_phase11_title_options.rs) | Created | Comprehensive integration test suite covering token parsing, config serde, UI construction, cursor insertion, and live title synchronization. |

---

## 3. Validation & Quality Gate Outcomes

### 3.1 Test Suites
All unit and integration test suites pass 100% headlessly with zero failures:
```bash
cargo test
cargo test --test test_phase11_title_options
```
- **Total Test Execution Count:** **147 passed; 0 failed; 0 ignored** across all targets.
- Headless GTK integration tests verify token expansion across all Terminal, Session, and Window scopes, serde backwards compatibility with legacy configuration files, Appearance preferences UI rows, popover menu button cursor-position insertion, dynamic window/session title propagation, and regression protection ensuring focus transitions preserve `${directory}` without wipeout.
- Fixed re-entrant `RefCell` borrowing conflict where action handlers in `TilixWindow` held `session.borrow_mut()` across title update notifications, causing `compute_and_apply_window_title` to fail `session_rc.try_borrow()` and fall back to an empty `TokenContext`. Changed all action invocations to `session.borrow()`, eliminated redundant `grab_focus` loops, and wired `/proc/<pid>/cwd` live fallback for reliable real-time directory retrieval.

### 3.2 Compiler & Linter Verification
- `cargo check --all-targets`: **0 errors, 0 warnings**
- `cargo clippy --all-targets -- -D warnings`: **0 violations, 0 warnings**

### 3.3 Documentation & Asset Hygiene
- Strictly verified zero references or links to temporary image files or screenshots in all documentation, specifications, tasks, results, and source code.

---

## 4. Residual Risks & Technical Debt

1. **Linux `/proc` Assumption for Process Names:**
   - *Observation:* Foreground process name resolution relies on `/proc` filesystem semantics.
   - *Mitigation:* On BSD or macOS systems without a mounted procfs, process tokens gracefully fall back to empty strings without crashing.
2. **Manual Per-Session Name Overrides:**
   - *Observation:* When users want to manually rename a specific session tab (e.g. via double-click or shortcut), an override flag on `SessionModel` will be needed to prevent `default_session_name` from overwriting the custom name.
   - *Mitigation:* Planned for the interactive session management follow-up milestone.
3. **Subshell OSC 7 Latency:**
   - *Observation:* Directory changes in rapid subshells emit OSC 7 escape sequences asynchronously.
   - *Mitigation:* Token evaluation updates on the next event loop tick without dropping events.

---

## 5. Future Milestones

1. **Phase 12 (Interactive Session Tab Renaming & Layout Templates):**
   - Support manual session renaming with persistent overrides and template save dialogs.
2. **Phase 13 (Terminal Context Menu Parity):**
   - Full context menu with URL copy, hyperlink opening, terminal split actions, and signal sending.
3. **Phase 14 (Packaging & Distribution Polish):**
   - Flatpak manifest, desktop launcher entries, appdata metainfo XML, and system theme synchronization.
