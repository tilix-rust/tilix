# Delivery Result Record: Phase 6 — Terminal Pane Title & Toolbar Visibility Controls

- **Document ID:** `RESULT-0006`
- **Task Reference:** [`docs/tasks/task-0006.md`](file:///playground/tilix/docs/tasks/task-0006.md)
- **Spec Reference:** [`docs/specs/spec-0006-pane-toolbar-terminal-title-2026-09-17.md`](file:///playground/tilix/docs/specs/spec-0006-pane-toolbar-terminal-title-2026-09-17.md)
- **Living Architecture Reference:** [`docs/architecture/overview.md`](file:///playground/tilix/docs/architecture/overview.md)
- **Date:** 2026-09-17
- **Status:** Completed & Verified (Approved by Review Gate)

---

## 1. Executive Summary

Following user feedback clarifying the dual toolbar/titlebar hierarchy of original Tilix (Direction C), Phase 6 delivers granular visibility controls for terminal pane header bars (`TerminalPane` Header) while maintaining top-level window style customizations from Phase 5:
1. **Terminal Pane Title & Toolbar Visibility (`PaneTitleStyle`):** Allows users to choose between `Normal` (displaying the pane header with terminal title, input sync toggle, split horizontal/vertical, and close buttons) and `None` (completely hiding the pane header for a clean, maximized terminal surface).
2. **Dynamic Auto-Hide When Single Terminal (`pane_title_show_when_single`):** When set to `false`, single-terminal tabs automatically hide the redundant pane header bar, reclaiming vertical space. As soon as the terminal is split into two or more panes, headers automatically reveal themselves to provide clear visual boundaries and mouse controls. Closing splits back to a single pane hides the header again automatically.
3. **Zero Widget Disruption:** All visibility transitions mutate `header.set_visible(...)` in place without destroying or reparenting terminal widgets. PTY child processes, terminal focus, and scrollback buffers are completely undisturbed.
4. **Independent Window & Pane Controls:** Top-level window style (`WindowStyle::Normal` vs `WindowStyle::HideToolbar`) and pane-level toolbar visibility (`PaneTitleStyle` and `pane_title_show_when_single`) operate completely independently, giving users total control over their terminal appearance.

---

## 2. Completed Work & Delivered Files

| File | Status | Description |
| :--- | :--- | :--- |
| [`src/model/config.rs`](file:///playground/tilix/src/model/config.rs) | Modified | Added `PaneTitleStyle` enum (`Normal`, `None`), extended `AppConfig` with `pane_title_style` and `pane_title_show_when_single`, established defaults, and added unit tests. |
| [`src/model/mod.rs`](file:///playground/tilix/src/model/mod.rs) | Modified | Re-exported `PaneTitleStyle` domain type. |
| [`src/ui/terminal_pane.rs`](file:///playground/tilix/src/ui/terminal_pane.rs) | Modified | Added `set_header_visible` and `is_header_visible` helper methods on `TerminalPane`. |
| [`src/ui/session_view.rs`](file:///playground/tilix/src/ui/session_view.rs) | Modified | Stored pane title settings, implemented `update_pane_headers_visibility` and `set_pane_title_settings`, and hooked into `rebuild_projection()` for dynamic split/close visibility updates. |
| [`src/ui/window.rs`](file:///playground/tilix/src/ui/window.rs) | Modified | Added `apply_pane_title_settings_to_all_sessions` for live broadcast across all open sessions. |
| [`src/ui/preferences.rs`](file:///playground/tilix/src/ui/preferences.rs) | Modified | Added "Terminal Title" preferences group in Appearance tab with "Title Style" `adw::ComboRow` and "Show title when single terminal" `adw::SwitchRow`, wiring reactive updates to `sync_and_save`. |
| [`tests/test_phase6_pane_toolbar.rs`](file:///playground/tilix/tests/test_phase6_pane_toolbar.rs) | Created | Headless integration test suite validating default values, serialization, legacy config compatibility, visibility simulation, and roundtrip file I/O. |
| [`docs/architecture/overview.md`](file:///playground/tilix/docs/architecture/overview.md) | Updated | Updated living architecture document to version 0.6.0, adding Section 17 detailing Terminal Pane Title & Toolbar Visibility Controls. |
| [`docs/specs/spec-0006-pane-toolbar-terminal-title-2026-09-17.md`](file:///playground/tilix/docs/specs/spec-0006-pane-toolbar-terminal-title-2026-09-17.md) | Created | Master specification for Phase 6. |
| [`docs/tasks/task-0006.md`](file:///playground/tilix/docs/tasks/task-0006.md) | Created | Task execution checklist for Phase 6. |

---

## 3. Validation & Quality Gate Outcomes

### 3.1 Test Suites
All unit, integration, and regression suites passed 100% headlessly with zero failures:
```bash
cargo test
```
- `unittests (src/main.rs)`: **67 passed; 0 failed**
- `tests/test_phase6_pane_toolbar.rs`: **57 passed; 0 failed**
- `tests/test_phase5_window_style_wide_handle.rs`: **56 passed; 0 failed**
- `tests/test_phase4_packaging_polish.rs`: **67 passed; 0 failed**
- `tests/test_phase3_domain.rs`: **62 passed; 0 failed**
- `tests/test_phase2_domain.rs`: **60 passed; 0 failed**
- **Total:** **369 tests passed; 0 failed; 0 ignored**

### 3.2 Compiler & Linter Verification
- `cargo check --all-targets`: **0 errors, 0 warnings**
- `cargo clippy --all-targets -- -D warnings`: **0 violations, 0 warnings**

### 3.3 Backwards & Forwards Compatibility Verification
- Deserialization of existing configuration files omitting `pane_title_style` or `pane_title_show_when_single` seamlessly populates `PaneTitleStyle::Normal` and `pane_title_show_when_single: true` via `#[serde(default)]`.
- Unknown future JSON properties are preserved or ignored without panics.

### 3.4 Terminal Accelerators Integrity
- All terminal operations (split right `Ctrl+Shift+R`, split down `Ctrl+Shift+D`, close pane `Ctrl+Shift+W`, sync input `Ctrl+Shift+I`) remain 100% operational when pane headers are hidden.

---

## 4. Residual Risks & Tech Debt

1. **Global Configuration Scope:**
   - `pane_title_style` and `pane_title_show_when_single` apply globally across all sessions and windows. Per-profile or per-tab overrides are not currently supported.
2. **Mouse Controls in None Mode:**
   - When `PaneTitleStyle::None` is active, visual click buttons on the pane header (split, close, sync) are hidden. Users rely on keyboard shortcuts or right-click context menus.

---

## 5. Future Milestones

- **Per-Profile Title Overrides:** Allow specific terminal profiles to override the global title visibility setting.
- **Session Layout Template Serialization:** Persist and reload custom multi-pane split topologies across application restarts.
