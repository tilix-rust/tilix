# Master Specification: Phase 9 — Complete Tilix Profile Customization Subsystem

- **Document ID:** `SPEC-0009`
- **Date:** 2026-09-17
- **Status:** Approved / Ready for Execution
- **Workflow Level:** Medium/Large (Phase 9 of Tilix Rewrite)
- **Preceding Specifications:**
  - [`docs/specs/spec-0001-rust-tilix-foundation-2026-09-15.md`](file:///playground/tilix/docs/specs/spec-0001-rust-tilix-foundation-2026-09-15.md)
  - [`docs/specs/spec-0002-sessions-sync-profiles-2026-09-15.md`](file:///playground/tilix/docs/specs/spec-0002-sessions-sync-profiles-2026-09-15.md)
  - [`docs/specs/spec-0003-wayland-quake-osc-dnd-2026-09-15.md`](file:///playground/tilix/docs/specs/spec-0003-wayland-quake-osc-dnd-2026-09-15.md)
  - [`docs/specs/spec-0005-window-style-wide-handle-2026-09-17.md`](file:///playground/tilix/docs/specs/spec-0005-window-style-wide-handle-2026-09-17.md)
  - [`docs/specs/spec-0006-pane-toolbar-terminal-title-2026-09-17.md`](file:///playground/tilix/docs/specs/spec-0006-pane-toolbar-terminal-title-2026-09-17.md)
  - [`docs/specs/spec-0007-custom-keybindings-manager-2026-09-17.md`](file:///playground/tilix/docs/specs/spec-0007-custom-keybindings-manager-2026-09-17.md)
  - [`docs/specs/spec-0008-pane-dnd-docking-detach-2026-09-17.md`](file:///playground/tilix/docs/specs/spec-0008-pane-dnd-docking-detach-2026-09-17.md)
- **Target Location:** `/playground/tilix`

---

## 1. Context & Motivation

In the original Tilix terminal emulator (written in D/GTK3), **Profiles** represent the core customizability mechanism for terminal panes. Each profile defines terminal behavior, aesthetics, shell command execution, scrolling semantics, compatibility bindings, visual badges, and intelligent automation rules.

Prior to Phase 9, Tilix Rust possessed only an embryonic `Profile` struct containing basic fields: `id`, `name`, `color_scheme`, `font`, `scrollback_lines`, `cursor_shape`, and `cursor_blink`. Multiple profile management was missing (only a single `default_profile` field existed in `AppConfig`), and the Preferences dialog contained a rudimentary Appearance page without profile management or tab organization.

Phase 9 implements the complete, production-grade Tilix Profile customization subsystem corresponding to the upstream `com.gexperts.Tilix.Profile` schema and `source/gx/tilix/prefeditor/profileeditor.d`.

---

## 2. Amended Documents & Scope

- **Amended Document:** [`docs/architecture/overview.md`](file:///playground/tilix/docs/architecture/overview.md)
- **Status of Living Architecture:** Active (Extended for Phase 9, version bumped to 0.9.0)

### 2.1 Affected Scope
- `src/model/profile.rs`:
  - Enums: `EraseBindingPreference`, `TextBlinkModePreference`, `TerminalBellPreference`, `ExitActionPreference`, `CjkWidthPreference`, `BadgePosition`, `TriggerAction`.
  - Structs: `ProfileSwitchRule`, `CustomHyperlinkRule`, `TriggerRule`, `TitleTokenContext`.
  - Comprehensive `Profile` struct with full field definitions for all 7 tabs.
  - Pure functions: `expand_title_format`, `expand_badge_format`, `matches_switch_rule`.
- `src/model/config.rs`:
  - Add `profiles: Vec<Profile>`, `default_profile_id: String`.
  - Retain `pub default_profile: Profile` for 100% backwards compatibility with legacy serde and Phase 3 tests.
  - Implement multi-profile management methods: `get_profile`, `get_profile_mut`, `get_default_profile`, `add_profile`, `duplicate_profile`, `delete_profile`, `set_default_profile`, `update_profile`.
  - Define `ProfileError`.
- `src/model/mod.rs`:
  - Re-export new profile enums, structs, and token expansion functions.
- `src/pty/shell.rs`:
  - Add `format_shell_argv0` for login shell prefixing (`-bash`).
  - Add `build_spawn_args` for custom command execution vs default shell.
- `src/ui/terminal_pane.rs`:
  - Add `gtk::Scrollbar` connected to `terminal.vadjustment()` and controlled by `profile.show_scrollbar`.
  - Add `badge_label: gtk::Label` to `self.overlay` with positioning and token rendering.
  - Implement full `apply_profile(&Profile)`: font, cell scales, text blink, bold is bright, bold color, cursor colors, highlight colors, transparency, dim unfocused opacity, erase bindings, CJK width, word selection characters, scrollback.
  - Implement exit action handling (`Close`, `Restart`, `Hold`).
  - Connect OSC 7 directory and title changes to automatic profile switching.
- `src/ui/preferences.rs`:
  - Introduce full Libadwaita "Profiles" preference page with profile management header bar (Add, Clone, Delete, Set Default).
  - Implement 7 tab groups for selected profile: General, Command, Color, Scrolling, Compatibility, Badge, Advanced.
  - Live preview and instantaneous reactive persistence.
- `src/ui/session_view.rs` & `src/ui/window.rs`:
  - Per-pane profile assignment and switching.
  - Global profile synchronization.
  - CSS styling for `.terminal-badge` and `.terminal-margin-line`.
- `tests/test_phase9_profile.rs`:
  - Comprehensive headless integration test suite.

### 2.2 Unaffected Scope
- LayoutTree binary split and directional docking algorithms (`src/model/layout.rs`).
- DND controllers and gesture event pipes (`src/ui/dnd.rs`).
- Keybindings configuration catalog (`src/model/keybindings.rs`).

---

## 3. Goals & Non-Goals

### Goals
1. **Full Upstream Schema Parity:** Implement 100% of the properties defined in upstream `com.gexperts.Tilix.Profile` across all 7 tabs plus profile management.
2. **Robust Multi-Profile CRUD:** Support adding, cloning, updating, deleting (with guard against deleting the last remaining profile), and setting default profiles.
3. **100% Backwards Compatibility:** Older JSON configuration files (v1 through v8) lacking `profiles` or `default_profile_id` must deserialize without errors, automatically promoting `default_profile` into the profile collection.
4. **Pure Domain Testability:** All token expansions (`${title}`, `${id}`, `${profile}`, `${directory}`, `${appName}`), automatic profile switching rules, shell argv formatting, and profile CRUD must be pure headless Rust functions testable in CI.
5. **Interactive GTK4/Libadwaita Profile Editor:** Clean, HIG-compliant UI with a profile selector header bar and 7 preference groups/subpages.
6. **Dynamic Live Terminal Reconfiguration:** Changes in the Profile editor apply immediately to open terminal panes without restarting running shell processes.

### Non-Goals
- Migrating profile storage to GNOME GSettings/dconf (Tilix Rust maintains standalone portable JSON config).
- Arbitrary scriptable LUA triggers (triggers support standard regex notification/badge/command actions).

---

## 4. Architecture & Design Decisions

### 4.1 Decision Gating: Facts, Decisions, Assumptions, Deferred Items

- **Facts:**
  - `vte4` crate (v0.10, feature `v0_76`) natively exposes `set_cell_width_scale`, `set_cell_height_scale`, `set_text_blink_mode`, `set_bold_is_bright`, `set_color_bold`, `set_color_cursor`, `set_color_cursor_foreground`, `set_color_highlight`, `set_color_highlight_foreground`, `set_backspace_binding`, `set_delete_binding`, `set_cjk_ambiguous_width`, `set_word_char_exceptions`, and `set_scrollback_lines`.
  - `vte4::Terminal` implements `gtk::Scrollable`, allowing an external `gtk::Scrollbar` to be attached via `terminal.vadjustment()`.
  - `TerminalPane` already utilizes `gtk::Overlay`, allowing non-targetable badge labels and margin guide lines to be overlaid cleanly without interfering with pointer events.
  - Existing tests in `tests/test_phase3_domain.rs` access `config.default_profile` directly.
- **Decisions:**
  - Maintain `pub default_profile: Profile` on `AppConfig` synchronized with `profiles` and `default_profile_id` to ensure zero breaking changes across all existing test suites.
  - Implement token expansion as a pure function: `expand_tokens(format_str: &str, context: &TitleTokenContext) -> String`.
  - Handle `ExitAction::Hold` by suppressing pane close, updating the header title with `[Process exited: code]`, and keeping the scrollback buffer inspectable.
  - Handle `ExitAction::Restart` by re-triggering child spawn on the same terminal widget.
  - Badge rendering uses a dedicated `gtk::Label` with CSS `.terminal-badge`, positioned according to `BadgePosition` with `set_can_target(false)`.
  - Unfocused terminal dimming uses `widget.set_opacity(1.0 - dim_transparency_percent / 100.0)`.
- **Assumptions:**
  - If a profile specifies `use_system_font = true`, Tilix falls back to system monospace font ("Monospace 11") if desktop font settings are not accessible in headless environments.
  - Hostname matching in automatic profile switching matches exact hostname or "*" wildcard.
- **Deferred Items:**
  - Import/export of individual profiles as `.tilixprofile` files (scheduled for future profile exchange milestone).

---

### 4.2 Data Models & Schema Design (`src/model/profile.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EraseBindingPreference {
    #[default]
    Auto,
    AsciiDelete,
    AsciiBackspace,
    DeleteSequence,
    Tty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TextBlinkModePreference {
    #[default]
    Never,
    Focused,
    Unfocused,
    Always,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TerminalBellPreference {
    None,
    #[default]
    Sound,
    Icon,
    IconSound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExitActionPreference {
    #[default]
    Close,
    Restart,
    Hold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CjkWidthPreference {
    #[default]
    Narrow,
    Wide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BadgePosition {
    Northwest,
    #[default]
    Northeast,
    Southwest,
    Southeast,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileSwitchRule {
    pub hostname: String,
    pub directory: String,
    pub profile_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomHyperlinkRule {
    pub name: String,
    pub pattern: String,
    pub uri: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TriggerAction {
    Notification { text: String },
    SetBadge { badge: String },
    RunCommand { command: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TriggerRule {
    pub name: String,
    pub pattern: String,
    pub action: TriggerAction,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    // Identity
    pub id: String,
    pub name: String,

    // General Tab
    pub terminal_title: String,
    pub default_size_columns: u32,
    pub default_size_rows: u32,
    pub cell_width_scale: f64,
    pub cell_height_scale: f64,
    pub draw_margin: u32,
    pub text_blink_mode: TextBlinkModePreference,
    pub allow_bold: bool,
    pub rewrap_on_resize: bool,
    pub use_system_font: bool,
    pub font: Option<String>,
    pub select_by_word_chars: String,
    pub cursor_shape: CursorShapePreference,
    pub cursor_blink: CursorBlinkPreference,
    pub terminal_bell: TerminalBellPreference,

    // Command Tab
    pub login_shell: bool,
    pub use_custom_command: bool,
    pub custom_command: String,
    pub exit_action: ExitActionPreference,

    // Color Tab
    pub color_scheme: ColorScheme,
    pub use_theme_colors: bool,
    pub background_transparency_percent: u32,
    pub dim_transparency_percent: u32,
    pub bold_color_set: bool,
    pub bold_color: Option<RgbColor>,
    pub bold_is_bright: bool,
    pub cursor_colors_set: bool,
    pub cursor_background_color: Option<RgbColor>,
    pub cursor_foreground_color: Option<RgbColor>,
    pub highlight_colors_set: bool,
    pub highlight_background_color: Option<RgbColor>,
    pub highlight_foreground_color: Option<RgbColor>,

    // Scrolling Tab
    pub show_scrollbar: bool,
    pub scroll_on_output: bool,
    pub scroll_on_keystroke: bool,
    pub scrollback_unlimited: bool,
    pub scrollback_lines: Option<i64>,

    // Compatibility Tab
    pub backspace_binding: EraseBindingPreference,
    pub delete_binding: EraseBindingPreference,
    pub encoding: String,
    pub cjk_utf8_ambiguous_width: CjkWidthPreference,

    // Badge Tab
    pub badge_text: String,
    pub badge_position: BadgePosition,
    pub badge_color_set: bool,
    pub badge_color: Option<RgbColor>,
    pub badge_use_system_font: bool,
    pub badge_font: Option<String>,

    // Advanced Tab
    pub automatic_switch: Vec<ProfileSwitchRule>,
    pub custom_hyperlinks: Vec<CustomHyperlinkRule>,
    pub triggers: Vec<TriggerRule>,
    pub notify_silence_enabled: bool,
    pub notify_silence_threshold: u32,
}
```

---

### 4.3 Profile Management Lifecycle (`src/model/config.rs`)

`AppConfig` manages the list of profiles and the active default:
```rust
pub struct AppConfig {
    pub default_profile: Profile,
    pub profiles: Vec<Profile>,
    pub default_profile_id: String,
    // other window/quake/keybinding fields...
}
```

#### Lifecycle Rules:
1. **Adding a Profile:** Appends the profile with a unique generated ID (e.g. UUID or `profile-<nanos>`).
2. **Duplicating a Profile:** Copies all settings from source profile, assigns a new unique ID, and names it `"<Source Name> (Copy)"`.
3. **Deleting a Profile:**
   - If `profiles.len() <= 1`, returns `Err(ProfileError::CannotDeleteLastProfile)`.
   - If deleting the default profile (`id == default_profile_id`), the default is automatically transferred to the first remaining profile in the list, and `default_profile` is updated.
4. **Setting Default Profile:** Sets `default_profile_id` and copies the corresponding profile to `self.default_profile`.

---

### 4.4 Token Expansion Engine

Pure function with zero GTK dependencies:
```rust
pub struct TitleTokenContext<'a> {
    pub id: u64,
    pub title: &'a str,
    pub profile_name: &'a str,
    pub directory: Option<&'a Path>,
    pub app_name: &'a str,
}

pub fn expand_title_format(format_str: &str, ctx: &TitleTokenContext) -> String;
pub fn expand_badge_format(format_str: &str, ctx: &TitleTokenContext) -> String;
```
Supported tokens:
- `${id}`: Pane ID number.
- `${title}`: Terminal title emitted by shell / OSC.
- `${profile}`: Profile visible name.
- `${directory}`: Current working directory basename or full path.
- `${appName}`: Fixed string `"Tilix"`.

---

### 4.5 PTY Shell Spawning & Process Execution (`src/pty/shell.rs`)

1. **Login Shell:** When `profile.login_shell` is true, argv[0] is formatted as `"-<shell_name>"` (e.g. `"-bash"`), triggering standard shell login initialization scripts (`/etc/profile`, `~/.bash_profile`).
2. **Custom Command:** When `profile.use_custom_command` is enabled and `custom_command` is non-empty, Tilix spawns the specified command using `/bin/sh -c "<custom_command>"`.

---

### 4.6 TerminalPane Integration & Overlay Rendering (`src/ui/terminal_pane.rs`)

1. **Scrollbar Widget:** In `TerminalPane::new`, a horizontal `gtk::Box` wraps `vte::Terminal` and a `gtk::Scrollbar::new(gtk::Orientation::Vertical, terminal.vadjustment())`. `scrollbar.set_visible(profile.show_scrollbar)` allows instant toggling.
2. **Badge Overlay:** A `gtk::Label` with CSS `.terminal-badge` is mounted in `self.overlay`. `set_can_target(false)` prevents pointer capture. Position is mapped to `halign` / `valign` (`Northwest`, `Northeast`, `Southwest`, `Southeast`). Text is token-expanded.
3. **Margin Guide Line:** A vertical separator overlay with class `.terminal-margin-line` is displayed when `profile.draw_margin > 0`.
4. **Exit Actions:**
   - `Close`: Invokes close callbacks (default).
   - `Restart`: Re-spawns shell process in place.
   - `Hold`: Keeps terminal open, presents exit code status in header, does not destroy pane.
5. **Dim Unfocused Opacity:** In `set_active(bool)`, if inactive and `dim_transparency_percent > 0`, applies `set_opacity(1.0 - dim / 100.0)`.

---

### 4.7 Preferences UI Architecture (`src/ui/preferences.rs`)

A dedicated "Profiles" page in `adw::PreferencesWindow`:
1. **Header Control:**
   - `adw::ComboRow` selecting from `config.profiles`.
   - "Add Profile", "Clone Profile", "Delete Profile", and "Set as Default" actions.
2. **7 Organized Preferences Groups/Tabs:**
   - **General Group:** Visible name, title format string, dimensions (cols/rows), cell width/height scale, margin column, text blink combo, allow bold, rewrap on resize, system font switch + font picker, word chars, cursor shape/blink, bell mode.
   - **Command Group:** Login shell switch, custom command switch + entry, exit action combo.
   - **Colors Group:** Presets dropdown, use theme colors switch, background transparency slider (0..100%), dim unfocused slider (0..100%), bold color override & bold is bright, cursor colors override, highlight colors override, 16 ANSI color palette editor with color buttons.
   - **Scrolling Group:** Show scrollbar switch, scroll on output switch, scroll on keystroke switch, unlimited scrollback switch, scrollback lines spin row.
   - **Compatibility Group:** Backspace binding combo, Delete binding combo, encoding combo, ambiguous CJK width combo.
   - **Badge Group:** Badge text entry, position combo, custom color switch + picker, system font switch + font entry.
   - **Advanced Group:** Automatic profile switching rule list (with Add/Remove buttons for Hostname, Directory, Profile ID), custom hyperlinks rule list, triggers rule list, silence notification switch + threshold spin.

---

## 5. Compatibility & Migration Strategy

- **Legacy JSON Deserialization:**
  Older config files containing only `default_profile` without `profiles` or `default_profile_id` deserialize cleanly via `#[serde(default)]`. During post-deserialization validation, if `profiles` is empty, it is populated with `vec![default_profile.clone()]`, and `default_profile_id` is set to `default_profile.id`.
- **Existing Tests:**
  `config.default_profile` remains directly accessible and mutated in tests, keeping all 114 existing unit and integration tests passing without modification.

---

## 6. Validation Strategy

1. **Unit Tests (Headless):**
   - Profile struct defaults, validation, and serialization.
   - Token expansion engine for title and badge.
   - Profile CRUD and deletion protections.
   - Automatic profile switching rule matching.
   - Shell spawn argv construction for login shell and custom command.
2. **GTK Headless Tests (via `run_gtk_test`):**
   - TerminalPane dynamic profile application (cell scales, scrollbar visibility, badge positioning, colors).
   - Exit action handling.
   - Preferences window profile page construction and tab rendering.
3. **Integration Test Suite:**
   - `tests/test_phase9_profile.rs` covering all Phase 9 features end-to-end.

---

## 7. Risk Analysis & Mitigation

| Risk | Severity | Mitigation |
|---|---|---|
| Incompatibility with existing `AppConfig` schema | High | Retain `default_profile` as a synchronized field and provide serde defaults for `profiles` and `default_profile_id`. |
| VTE font scaling or rendering artifacts | Medium | Clamp `cell_width_scale` and `cell_height_scale` strictly between 1.0 and 2.0. |
| Pointer capture by badge overlay | Medium | Set `set_can_target(false)` on the badge label widget in the GTK overlay. |
| Accidental deletion of the only profile | High | Enforce `CannotDeleteLastProfile` in `AppConfig::delete_profile` and disable the delete button in UI when only 1 profile exists. |
