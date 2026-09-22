# Delivery Result Record: Phase 18 — OSC 8 Hyperlinks & Upstream Parity

- **Document ID:** `RESULT-0018`
- **Task Reference:** [`docs/tasks/task-0018.md`](file:///playground/tilix/docs/tasks/task-0018.md)
- **Spec Reference:** [`docs/specs/spec-0018-osc8-hyperlinks-2026-09-22.md`](file:///playground/tilix/docs/specs/spec-0018-osc8-hyperlinks-2026-09-22.md)
- **Living Architecture Reference:** [`docs/architecture/overview.md`](file:///playground/tilix/docs/architecture/overview.md) (Section 30)
- **Date:** 2026-09-22
- **Status:** Completed & Verified (Approved by Review Gate)

---

## 1. Executive Summary

Phase 18 achieves full upstream Tilix parity for explicit **OSC 8 terminal hyperlinks** (`\x1b]8;id=...;uri\x1b\\text\x1b]8;;\x1b\\`), modernizing Tilix Rust with native VTE hyperlinking, pointer interaction, security validation, and dynamic context menu controls:

1. **Domain Model & Configuration Compatibility:**
   - Extended `Profile` in [`src/model/profile.rs`](file:///playground/tilix/src/model/profile.rs) with `pub allow_hyperlinks: bool` (defaulting to `true`).
   - Implemented Serde default provider `#[serde(default = "default_true")]`, ensuring 100% backward compatibility with legacy configuration files lacking the key.
2. **VTE Terminal Integration:**
   - Initialized `terminal.set_allow_hyperlink(profile.allow_hyperlinks)` on pane creation in [`src/ui/terminal_pane.rs`](file:///playground/tilix/src/ui/terminal_pane.rs) and synchronized state dynamically via `apply_profile_to_widgets`.
   - Verified that `Osc52StreamParser` passes OSC 8 sequences through unaltered with 0 spurious clipboard events.
3. **Pointer Interaction & Primary Activation:**
   - Attached a `gtk::GestureClick` controller to `terminal` in `PropagationPhase::Capture`.
   - Intercepted `Ctrl + Left-Click` (Primary button with `CONTROL_MASK`), extracting target URIs via `terminal.check_hyperlink_at(x, y)` and safely launching them via the desktop's default URI handler (`gio::AppInfo::launch_default_for_uri`).
   - Claimed event sequence (`EventSequenceState::Claimed`) on activation to prevent spurious terminal text selection.
4. **Upstream Security Gating for `file://` URIs:**
   - Implemented `TerminalPane::is_safe_file_uri(uri)` matching upstream Tilix security behavior:
     - Allows local empty-host paths (`file:///path`) and loopback (`file://localhost/path`).
     - Allows host-addressed paths (`file://<hostname>/path`) strictly if `<hostname>` matches `glib::host_name()`.
     - Blocks remote hostnames and SMB network shares (`file://attacker.com/...`, `file://smb-server/...`), logging a security warning.
5. **Dynamic Context Menu & Window Actions:**
   - Right-clicking (Secondary button) checks pointer coordinates for active hyperlinks; when present, it stores the URI in `current_hyperlink_uri` and dynamically builds the context menu with prepended **"Open Link"** and **"Copy Link Address"** actions.
   - When no link is under pointer, restores the standard context menu.
   - Registered `win.open-link` and `win.copy-link-address` actions on [`TilixWindow`](file:///playground/tilix/src/ui/window.rs), dispatching through [`SessionView`](file:///playground/tilix/src/ui/session_view.rs) to the active pane.
6. **Preferences UI Parity:**
   - Added an `adw::SwitchRow` for **"Allow hyperlinks (OSC 8)"** under the Profile Compatibility tab in [`src/ui/preferences.rs`](file:///playground/tilix/src/ui/preferences.rs), wired to live synchronization.
7. **Comprehensive Automated Verification:**
   - Delivered 8/8 passing integration tests in [`tests/test_phase18_osc8_hyperlinks.rs`](file:///playground/tilix/tests/test_phase18_osc8_hyperlinks.rs).
   - All 319 unit and integration tests across the repository pass headlessly with zero regressions.

---

## 2. Completed Work & Delivered Files

| File | Status | Description |
| :--- | :--- | :--- |
| [`src/model/profile.rs`](file:///playground/tilix/src/model/profile.rs) | Modified | Added `allow_hyperlinks: bool` to `Profile` with serde default helper, default initialization, and unit tests. |
| [`src/ui/terminal_pane.rs`](file:///playground/tilix/src/ui/terminal_pane.rs) | Modified | Configured VTE `set_allow_hyperlink`, pointer tracking with `current_hyperlink_uri`, `is_safe_file_uri`, `launch_uri_safe`, `GestureClick` for Ctrl+Left-Click and Right-Click, dynamic `build_context_menu`, and pane-level link methods. |
| [`src/ui/session_view.rs`](file:///playground/tilix/src/ui/session_view.rs) | Modified | Implemented `open_link_active()` and `copy_link_address_active()` on `SessionView`. |
| [`src/ui/window.rs`](file:///playground/tilix/src/ui/window.rs) | Modified | Registered actions `win.open-link` and `win.copy-link-address` on `TilixWindow`. |
| [`src/ui/preferences.rs`](file:///playground/tilix/src/ui/preferences.rs) | Modified | Added `adw::SwitchRow` for "Allow hyperlinks (OSC 8)" under Profile Compatibility tab with synchronization. |
| [`docs/architecture/overview.md`](file:///playground/tilix/docs/architecture/overview.md) | Modified | Version bumped to 0.18.0; updated test matrix to 16 suites; added Section 30 detailing OSC 8 Hyperlinks & Upstream Parity architecture. |
| [`docs/specs/spec-0018-osc8-hyperlinks-2026-09-22.md`](file:///playground/tilix/docs/specs/spec-0018-osc8-hyperlinks-2026-09-22.md) | Created | Master architectural specification for Phase 18. |
| [`docs/tasks/task-0018.md`](file:///playground/tilix/docs/tasks/task-0018.md) | Created | Task execution checklist, fully completed and checked off. |
| [`tests/test_phase18_osc8_hyperlinks.rs`](file:///playground/tilix/tests/test_phase18_osc8_hyperlinks.rs) | Created | Comprehensive integration test suite covering 8 hyperlink scenarios. |
| [`docs/results/result-0018.md`](file:///playground/tilix/docs/results/result-0018.md) | Created | Delivery result record synthesized and archived directly by orchestrator. |

---

## 3. Validation & Quality Gate Outcomes

### 3.1 Compilation & Type Checking
- `cargo check --all-targets`: Passed cleanly with **0 errors, 0 warnings**.

### 3.2 Automated Test Execution
- `cargo test --test test_phase18_osc8_hyperlinks`:
  - `test_phase18_profile_allow_hyperlinks_default` ... **ok**
  - `test_phase18_osc8_stream_parser_passthrough` ... **ok**
  - `test_phase18_is_safe_file_uri_validation` ... **ok**
  - `test_phase18_profile_serde_backward_compat` ... **ok**
  - `test_phase18_profile_serde_roundtrip` ... **ok**
  - `test_phase18_context_menu_dynamic_link_section` ... **ok**
  - `test_phase18_vte_allow_hyperlinks_sync` ... **ok**
  - `test_phase18_window_hyperlink_actions_registered` ... **ok**
  - **Result:** 8 passed; 0 failed; 0 ignored in 0.00s.

### 3.3 Workspace Regression Suite
- Running `cargo test`:
  - `src/lib.rs` (Unit tests): 189 passed; 0 failed
  - `tests/test_phase2_domain.rs`: 5 passed; 0 failed
  - `tests/test_phase3_domain.rs`: 4 passed; 0 failed
  - `tests/test_phase4_packaging_polish.rs`: 9 passed; 0 failed
  - `tests/test_phase5_window_style_wide_handle.rs`: 8 passed; 0 failed
  - `tests/test_phase6_pane_toolbar.rs`: 9 passed; 0 failed
  - `tests/test_phase7_keybindings.rs`: 8 passed; 0 failed
  - `tests/test_phase8_dnd.rs`: 9 passed; 0 failed
  - `tests/test_phase9_profile.rs`: 8 passed; 0 failed
  - `tests/test_phase10_profile_ui_parity.rs`: 11 passed; 0 failed
  - `tests/test_phase11_title_options.rs`: 11 passed; 0 failed
  - `tests/test_phase12_window_geometry.rs`: 10 passed; 0 failed
  - `tests/test_phase13_zoom_and_alt_drag.rs`: 9 passed; 0 failed
  - `tests/test_phase14_osc52_and_clipboard.rs`: 14 passed; 0 failed
  - `tests/test_phase15_compact_mode.rs`: 9 passed; 0 failed
  - `tests/test_phase17_transparency.rs`: 8 passed; 0 failed
  - `tests/test_phase18_osc8_hyperlinks.rs`: 8 passed; 0 failed
  - **Total Test Suite:** **319 passed; 0 failed; 0 ignored.**

### 3.4 Linter & Code Hygiene
- `cargo clippy --lib --test test_phase18_osc8_hyperlinks -- -D warnings`: Passed cleanly with **0 violations, 0 warnings**.
- `cargo clippy --bin tilix -- -D warnings`: Passed cleanly with **0 violations, 0 warnings**.

---

## 4. Residual Risks & Tech Debt

| Item | Severity | Notes | Mitigation Strategy |
| :--- | :--- | :--- | :--- |
| **Desktop URI Handler Availability** | Low | `gio::AppInfo::launch_default_for_uri` relies on the system desktop environment (e.g. `xdg-open` or DBus portal `org.freedesktop.portal.OpenURI`). In minimal headless containers without a portal or browser installed, launch requests return an error. | Errors are handled gracefully and logged without crashing or panicking the application. |
| **Regex Custom Links Integration** | Low | The profile domain model contains `custom_hyperlinks: Vec<CustomHyperlinkRule>` for regex matching on terminal text. This operates independently of OSC 8 explicit escape sequence hyperlinks. | Scheduled for a future regex pattern match integration milestone. |

---

## 5. Future Milestones

1. **Phase 19 (Custom Hyperlink Regex Matching):**
   - Connect `Profile.custom_hyperlinks` to VTE's PCRE2 regex engine (`vte::Terminal::match_add_regex`), allowing custom regex pattern matches (such as issue tracker IDs or commit hashes) to be activated alongside OSC 8 hyperlinks.
2. **Phase 20 (Session State Persistence & Restoration):**
   - Save and restore multi-session split layouts and working directories across application restarts.
