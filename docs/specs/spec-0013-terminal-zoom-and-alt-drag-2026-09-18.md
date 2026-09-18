# Master Specification: Phase 13 — Terminal Font Zoom & Alt Drag-and-Drop Pane Docking

- **Document ID:** `SPEC-0013`
- **Date:** 2026-09-18
- **Status:** Proposed / Ready for Execution
- **Workflow Level:** Medium/Large (Phase 13 of Tilix Rewrite)
- **Preceding Specifications:**
  - [`docs/specs/spec-0001-rust-tilix-foundation-2026-09-15.md`](file:///playground/tilix/docs/specs/spec-0001-rust-tilix-foundation-2026-09-15.md)
  - [`docs/specs/spec-0002-sessions-sync-profiles-2026-09-15.md`](file:///playground/tilix/docs/specs/spec-0002-sessions-sync-profiles-2026-09-15.md)
  - [`docs/specs/spec-0003-wayland-quake-osc-dnd-2026-09-15.md`](file:///playground/tilix/docs/specs/spec-0003-wayland-quake-osc-dnd-2026-09-15.md)
  - [`docs/specs/spec-0004-packaging-distribution-polish-2026-09-16.md`](file:///playground/tilix/docs/specs/spec-0004-packaging-distribution-polish-2026-09-16.md)
  - [`docs/specs/spec-0005-window-style-wide-handle-2026-09-17.md`](file:///playground/tilix/docs/specs/spec-0005-window-style-wide-handle-2026-09-17.md)
  - [`docs/specs/spec-0006-pane-toolbar-terminal-title-2026-09-17.md`](file:///playground/tilix/docs/specs/spec-0006-pane-toolbar-terminal-title-2026-09-17.md)
  - [`docs/specs/spec-0007-custom-keybindings-manager-2026-09-17.md`](file:///playground/tilix/docs/specs/spec-0007-custom-keybindings-manager-2026-09-17.md)
  - [`docs/specs/spec-0008-pane-dnd-docking-detach-2026-09-17.md`](file:///playground/tilix/docs/specs/spec-0008-pane-dnd-docking-detach-2026-09-17.md)
  - [`docs/specs/spec-0009-profile-customization-complete-2026-09-17.md`](file:///playground/tilix/docs/specs/spec-0009-profile-customization-complete-2026-09-17.md)
  - [`docs/specs/spec-0010-profile-preferences-ui-parity-2026-09-17.md`](file:///playground/tilix/docs/specs/spec-0010-profile-preferences-ui-parity-2026-09-17.md)
  - [`docs/specs/spec-0011-session-and-application-title-options-2026-09-18.md`](file:///playground/tilix/docs/specs/spec-0011-session-and-application-title-options-2026-09-18.md)
  - [`docs/specs/spec-0012-dynamic-window-geometry-default-size-2026-09-18.md`](file:///playground/tilix/docs/specs/spec-0012-dynamic-window-geometry-default-size-2026-09-18.md)
- **Target Location:** `/playground/tilix`

---

## 1. Context & Motivation

In previous phases, Tilix Rust implemented core multiplexing, session management, profiles, keybindings management, drag-and-drop pane docking, and dynamic window geometry. Two critical usability features from upstream Tilix remain unaddressed:

1. **Terminal Font Zooming:**
   Users need the ability to quickly increase, decrease, or reset terminal font scaling on the fly using standard keyboard shortcuts (`Ctrl++`, `Ctrl+-`, `Ctrl+0`) and mouse wheel interactions (`Ctrl+ScrollUp`, `Ctrl+ScrollDown`). Currently, changing font size requires modifying the profile preferences. Dynamic font scaling via VTE's native `font-scale` property allows instant resizing for presentations, readability, or varying monitor distances without modifying persistent profile configurations.
   Moreover, mouse wheel scrolling with `Ctrl` must be intercepted before reaching VTE's buffer scrolling, ensuring smooth zoom adjustments without erratically scrolling the terminal history.

2. **Alt + Left Mouse Button Drag-and-Drop Pane Docking (Upstream Issue #1335):**
   In Phase 8 ([`SPEC-0008`](file:///playground/tilix/docs/specs/spec-0008-pane-dnd-docking-detach-2026-09-17.md)), pane dragging was implemented via a `gtk::DragSource` attached exclusively to the pane header bar (`TerminalPane.header`). However, when pane title bars are hidden (`pane_title_style: none` or when only a single pane exists without headers), users cannot drag panes to rearrange or dock them. Furthermore, dragging directly from the terminal area is far more ergonomic.
   Directly attaching an unrestricted drag source to the `vte::Terminal` widget would break terminal text selection and URL clicking. Following upstream Tilix's proven design, Phase 13 introduces `Alt + Left-click` drag initiation on the terminal widget: holding `Alt` allows dragging the pane from anywhere inside the terminal, while normal left-clicks and drags continue to handle terminal text selection and interaction without interference.

---

## 2. Amended Documents & Scope

- **Amended Document:** [`docs/architecture/overview.md`](file:///playground/tilix/docs/architecture/overview.md)
- **Status of Living Architecture:** Active (Version bumped to 0.13.0 for Phase 13)

### 2.1 Affected Scope
- `src/model/keybindings.rs`:
  - Add 3 zoom action definitions to `ACTION_CATALOG` under `ActionCategory::ViewAndSettings`:
    - `win.zoom-in` ("Zoom In", default accels: `<Primary>plus`, `<Primary>equal`, `<Primary>KP_Add`).
    - `win.zoom-out` ("Zoom Out", default accels: `<Primary>minus`, `<Primary>KP_Subtract`).
    - `win.zoom-normal` ("Normal Size", default accels: `<Primary>0`, `<Primary>KP_0`).
  - Update action catalog count from 24 to 27.
  - Update internal unit tests for catalog completeness, category distribution, and accelerator resolution.
- `src/ui/terminal_pane.rs`:
  - Introduce zoom constants: `ZOOM_STEP = 0.1`, `ZOOM_MIN = 0.2`, `ZOOM_MAX = 5.0`, `ZOOM_NORMAL = 1.0`.
  - Add public methods: `zoom_in(&self)`, `zoom_out(&self)`, `zoom_normal(&self)`, `font_scale(&self) -> f64`.
  - In `TerminalPane::new`, attach `gtk::EventControllerScroll` (vertical flags, `Capture` phase) to `terminal`. Intercept vertical scrolling when `CONTROL_MASK` is set (without Shift or Alt), zoom in/out, and return `glib::Propagation::Stop`.
  - Update `setup_drag_source(&self)` to attach drag sources to both `self.header` (`require_alt = false`) and `self.terminal` (`require_alt = true`).
- `src/ui/dnd.rs`:
  - Update `setup_pane_drag_source(widget: &impl IsA<gtk::Widget>, pane: TerminalPane, require_alt: bool)`.
  - When `require_alt` is true:
    - Connect `drag_source.connect_begin` to check `current_event_state()`. If `ALT_MASK` is absent, deny the gesture sequence via `gesture.set_sequence_state(seq, gtk::EventSequenceState::Denied)`.
    - Connect `drag_source.connect_prepare` to ensure `source.current_event_state()` contains `ALT_MASK`; return `None` otherwise.
- `src/ui/session_view.rs`:
  - Add `active_pane(&self) -> Option<TerminalPane>`.
  - Add `zoom_in_active(&self)`, `zoom_out_active(&self)`, `zoom_normal_active(&self)` dispatching to active pane.
- `src/ui/window.rs`:
  - In `setup_actions(&self)`, register `gio::SimpleAction` for `zoom-in`, `zoom-out`, and `zoom-normal`, invoking active session zoom methods.
- `tests/test_phase7_keybindings.rs`:
  - Update catalog length assertions from 24 to 27 and `ViewAndSettings` category count from 2 to 5.
- `tests/test_phase13_zoom_and_alt_drag.rs` (New Integration Suite):
  - Headless integration test suite verifying font zoom scaling, clamping, precision, shortcut resolution, action registration, scroll interception, and Alt-drag gating.

### 2.2 Unaffected Scope
- `src/pty/*`: Shell spawning and PTY data streams remain unchanged.
- `src/model/layout.rs`: Split tree topology, docking math, and layout balancing remain unchanged.
- `src/model/profile.rs` and `src/model/config.rs`: Profile serialization schema remains unchanged.
- `src/ui/preferences.rs`: Preferences UI dialog structure remains unchanged.
- `src/ui/geometry.rs`: Window geometry calculation remains unchanged.

---

## 3. Goals & Non-Goals

### Goals
1. **Accurate Font Zoom Operations:**
   - Incremental font scale changes in steps of `0.1` (`ZOOM_STEP`).
   - Clamping within safe bounds: `[0.2, 5.0]` (`ZOOM_MIN` to `ZOOM_MAX`).
   - Clean reset to `1.0` (`ZOOM_NORMAL`).
   - Arithmetic precision preservation using `((scale * 10.0).round() / 10.0)` to eliminate IEEE-754 drift.
2. **Scroll Event Capture Interception:**
   - Intercept vertical scroll events on `vte::Terminal` in the `Capture` phase when `Control` is pressed without `Shift` or `Alt`.
   - Prevent scrollback movement (`glib::Propagation::Stop`) while zooming.
   - Transparently pass unadorned scroll events (`glib::Propagation::Proceed`) to VTE for regular terminal buffer scrolling.
3. **Comprehensive Shortcut Catalog Integration:**
   - Catalog actions `win.zoom-in`, `win.zoom-out`, and `win.zoom-normal` in `ActionCategory::ViewAndSettings`.
   - Default primary accelerators (`<Primary>plus`, `<Primary>minus`, `<Primary>0`) along with standard aliases (`<Primary>equal`, `<Primary>KP_Add`, `<Primary>KP_Subtract`, `<Primary>KP_0`).
   - Live accelerator registration via `app.set_accels_for_action`.
4. **Alt + Left Click Drag-and-Drop Docking (Issue #1335):**
   - Enable pane dragging directly from the terminal area when holding `Alt`.
   - Immediately deny the gesture (`EventSequenceState::Denied`) when `Alt` is not pressed, completely preserving normal text selection, word double-clicking, and URL interaction.
   - Maintain header bar dragging without requiring `Alt`.
5. **Session-Level Active Dispatch:**
   - Route window-level zoom actions to the currently active pane in the active session.
6. **100% Automated Headless Test Coverage:**
   - Unit and integration tests for zoom mechanics, action catalog resolution, window action dispatch, and drag source modifier gating.

### Non-Goals
- Persisting zoom scale across application restarts or writing zoom level back to `Profile` (zoom is ephemeral session state).
- Altering the default mouse scroll line count for standard terminal buffer navigation.
- Configurable modifier keys for terminal pane drag (Alt is the established standard across desktop multiplexers).

---

## 4. Architecture & Design Decisions

### 4.1 Decision Gating: Facts, Decisions, Assumptions, Deferred Items

- **Facts:**
  - `vte::Terminal` in the `vte4` crate provides `font_scale(&self) -> f64` and `set_font_scale(&self, scale: f64)`.
  - `gtk::EventControllerScroll` in GTK4 captures scroll events and provides `current_event_state() -> gdk::ModifierType`.
  - Setting `gtk::PropagationPhase::Capture` ensures controller callbacks fire before child widgets (VTE) process the event in the `Target`/`Bubble` phase.
  - `gtk::DragSource` is a `gtk::Gesture`, allowing sequence state rejection via `gesture.set_sequence_state(seq, gtk::EventSequenceState::Denied)` and `source.current_event_state()`.
  - `ACTION_CATALOG` in `src/model/keybindings.rs` currently contains 24 actions, with 2 in `ActionCategory::ViewAndSettings`.
  - `app.set_accels_for_action` in `src/ui/window.rs` dynamically maps shortcuts from `ACTION_CATALOG` across all open windows.
- **Decisions:**
  - **Zoom Step & Bounds:**
    - `ZOOM_STEP = 0.1`
    - `ZOOM_MIN = 0.2`
    - `ZOOM_MAX = 5.0`
    - `ZOOM_NORMAL = 1.0`
    - Precision rounding: `((next * 10.0).round() / 10.0).clamp(ZOOM_MIN, ZOOM_MAX)` to prevent values such as `1.1000000000000001`.
  - **Scroll Propagation Control:**
    - When `CONTROL_MASK` is set without `SHIFT_MASK` or `ALT_MASK`:
      - `dy < 0.0` (scroll up) -> `zoom_in()`, return `glib::Propagation::Stop`.
      - `dy > 0.0` (scroll down) -> `zoom_out()`, return `glib::Propagation::Stop`.
      - `dy == 0.0` -> return `glib::Propagation::Proceed`.
    - Otherwise -> return `glib::Propagation::Proceed`.
  - **Alt Drag Gating:**
    - Parameterize `setup_pane_drag_source` with `require_alt: bool`.
    - When `require_alt` is true:
      - In `connect_begin`: check `gesture.current_event_state().contains(ALT_MASK)`. If false, set sequence state to `Denied`.
      - In `connect_prepare`: check `source.current_event_state().contains(ALT_MASK)`. If false, return `None`.
    - Terminal widget uses `require_alt = true`; header bar widget uses `require_alt = false`.
- **Assumptions:**
  - `<Primary>` represents `Control` on Linux/X11/Wayland desktop platforms.
  - Terminal selection gestures in VTE function without interruption when `Alt` is not pressed because `DragSource` denies the event sequence before dragging starts.
- **Deferred Items:**
  - Per-profile default zoom factor configuration (future milestone).
  - Configurable drag modifier keys (future milestone).

---

## 5. Detailed Component Specifications

### 5.1 `TerminalPane` (`src/ui/terminal_pane.rs`)

#### 5.1.1 Constants
```rust
pub const ZOOM_STEP: f64 = 0.1;
pub const ZOOM_MIN: f64 = 0.2;
pub const ZOOM_MAX: f64 = 5.0;
pub const ZOOM_NORMAL: f64 = 1.0;
```

#### 5.1.2 Public Methods
- `pub fn font_scale(&self) -> f64`:
  Returns `self.terminal.font_scale()`.
- `pub fn zoom_in(&self)`:
  Computes `(((self.font_scale() + ZOOM_STEP) * 10.0).round() / 10.0).clamp(ZOOM_MIN, ZOOM_MAX)` and applies via `self.terminal.set_font_scale(new_scale)`.
- `pub fn zoom_out(&self)`:
  Computes `(((self.font_scale() - ZOOM_STEP) * 10.0).round() / 10.0).clamp(ZOOM_MIN, ZOOM_MAX)` and applies via `self.terminal.set_font_scale(new_scale)`.
- `pub fn zoom_normal(&self)`:
  Resets font scale via `self.terminal.set_font_scale(ZOOM_NORMAL)`.

#### 5.1.3 EventControllerScroll Setup
In `TerminalPane::new`:
```rust
let scroll_controller = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
scroll_controller.set_propagation_phase(gtk::PropagationPhase::Capture);
let term_ref = terminal.clone();
scroll_controller.connect_scroll(move |controller, _dx, dy| {
    let state = controller.current_event_state();
    let has_ctrl = state.contains(gtk::gdk::ModifierType::CONTROL_MASK);
    let has_shift = state.contains(gtk::gdk::ModifierType::SHIFT_MASK);
    let has_alt = state.contains(gtk::gdk::ModifierType::ALT_MASK);

    if has_ctrl && !has_shift && !has_alt {
        if dy < 0.0 {
            let current = term_ref.font_scale();
            let next = (((current + ZOOM_STEP) * 10.0).round() / 10.0).clamp(ZOOM_MIN, ZOOM_MAX);
            term_ref.set_font_scale(next);
            glib::Propagation::Stop
        } else if dy > 0.0 {
            let current = term_ref.font_scale();
            let next = (((current - ZOOM_STEP) * 10.0).round() / 10.0).clamp(ZOOM_MIN, ZOOM_MAX);
            term_ref.set_font_scale(next);
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    } else {
        glib::Propagation::Proceed
    }
});
terminal.add_controller(scroll_controller);
```

#### 5.1.4 Dual Drag Source Attachment
In `TerminalPane::setup_drag_source`:
```rust
pub fn setup_drag_source(&self) {
    if self.drag_source_initialized.get() {
        return;
    }
    self.drag_source_initialized.set(true);
    crate::ui::dnd::setup_pane_drag_source(&self.header, self.clone(), false);
    crate::ui::dnd::setup_pane_drag_source(&self.terminal, self.clone(), true);
}
```

---

### 5.2 Drag and Drop Engine (`src/ui/dnd.rs`)

Update signature and add modifier gating:
```rust
pub fn setup_pane_drag_source(
    widget: &impl IsA<gtk::Widget>,
    pane: TerminalPane,
    require_alt: bool,
) {
    let drag_source = gtk::DragSource::new();
    drag_source.set_actions(gtk::gdk::DragAction::MOVE);
    let pane_id = pane.pane_id();
    let id_val = pane_id.0;
    let pane_clone = pane;

    if require_alt {
        drag_source.connect_begin(move |gesture, seq| {
            let state = gesture.current_event_state();
            if !state.contains(gtk::gdk::ModifierType::ALT_MASK) {
                if let Some(s) = seq {
                    gesture.set_sequence_state(s, gtk::EventSequenceState::Denied);
                } else {
                    gesture.set_state(gtk::EventSequenceState::Denied);
                }
            }
        });
    }

    drag_source.connect_prepare(move |source, _x, _y| {
        if require_alt {
            let state = source.current_event_state();
            if !state.contains(gtk::gdk::ModifierType::ALT_MASK) {
                return None;
            }
        }

        let session_widget = source
            .widget()
            .and_then(|w| crate::ui::window::find_session_widget(&w))
            .map(|w| w.downgrade())
            .unwrap_or_default();

        let active = ActivePaneDrag {
            pane_id,
            source_session_widget: session_widget,
            pane: pane_clone.clone(),
        };
        set_active_pane_drag(Some(active));

        Some(gtk::gdk::ContentProvider::for_value(&id_val.to_value()))
    });

    drag_source.connect_drag_cancel(move |_source, _drag, reason| {
        if reason == gtk::gdk::DragCancelReason::NoTarget
            && crate::ui::window::detach_drag_to_new_window()
        {
            return true;
        }
        false
    });

    drag_source.connect_drag_end(move |_source, _drag, _delete_data| {
        clear_active_pane_drag();
    });

    widget.add_controller(drag_source);
}
```

---

### 5.3 Keybindings Catalog (`src/model/keybindings.rs`)

Add 3 action definitions to `ACTION_CATALOG` in `ActionCategory::ViewAndSettings`:
```rust
ActionShortcutDef {
    id: "win.zoom-in",
    title: "Zoom In",
    description: "Increase terminal font size",
    category: ActionCategory::ViewAndSettings,
    default_accels: &["<Primary>plus", "<Primary>equal", "<Primary>KP_Add"],
},
ActionShortcutDef {
    id: "win.zoom-out",
    title: "Zoom Out",
    description: "Decrease terminal font size",
    category: ActionCategory::ViewAndSettings,
    default_accels: &["<Primary>minus", "<Primary>KP_Subtract"],
},
ActionShortcutDef {
    id: "win.zoom-normal",
    title: "Normal Size",
    description: "Reset terminal font size to default",
    category: ActionCategory::ViewAndSettings,
    default_accels: &["<Primary>0", "<Primary>KP_0"],
},
```

---

### 5.4 `SessionView` (`src/ui/session_view.rs`)

Add active pane lookup and zoom dispatchers:
```rust
pub fn active_pane(&self) -> Option<TerminalPane> {
    let id = self.active_pane_id()?;
    self.panes.borrow().get(&id).cloned()
}

pub fn zoom_in_active(&self) {
    if let Some(pane) = self.active_pane() {
        pane.zoom_in();
    }
}

pub fn zoom_out_active(&self) {
    if let Some(pane) = self.active_pane() {
        pane.zoom_out();
    }
}

pub fn zoom_normal_active(&self) {
    if let Some(pane) = self.active_pane() {
        pane.zoom_normal();
    }
}
```

---

### 5.5 `TilixWindow` (`src/ui/window.rs`)

In `setup_actions(&self)`, register the 3 zoom window actions:
```rust
// Zoom In
{
    let action = gio::SimpleAction::new("zoom-in", None);
    let tab_view_weak = self.tab_view.downgrade();
    let sessions = Rc::clone(&self.sessions);
    action.connect_activate(move |_, _| {
        let Some(tv) = tab_view_weak.upgrade() else { return; };
        let Some(page) = tv.selected_page() else { return; };
        let session_opt = sessions.borrow().get(&page).cloned();
        if let Some(session) = session_opt {
            session.borrow().zoom_in_active();
        }
    });
    self.window.add_action(&action);
}

// Zoom Out
{
    let action = gio::SimpleAction::new("zoom-out", None);
    let tab_view_weak = self.tab_view.downgrade();
    let sessions = Rc::clone(&self.sessions);
    action.connect_activate(move |_, _| {
        let Some(tv) = tab_view_weak.upgrade() else { return; };
        let Some(page) = tv.selected_page() else { return; };
        let session_opt = sessions.borrow().get(&page).cloned();
        if let Some(session) = session_opt {
            session.borrow().zoom_out_active();
        }
    });
    self.window.add_action(&action);
}

// Zoom Normal
{
    let action = gio::SimpleAction::new("zoom-normal", None);
    let tab_view_weak = self.tab_view.downgrade();
    let sessions = Rc::clone(&self.sessions);
    action.connect_activate(move |_, _| {
        let Some(tv) = tab_view_weak.upgrade() else { return; };
        let Some(page) = tv.selected_page() else { return; };
        let session_opt = sessions.borrow().get(&page).cloned();
        if let Some(session) = session_opt {
            session.borrow().zoom_normal_active();
        }
    });
    self.window.add_action(&action);
}
```

---

## 6. TDD & Validation Strategy

### 6.1 Test Suites & Coverage
1. **Catalog & Keybinding Tests (`src/model/keybindings.rs` & `tests/test_phase7_keybindings.rs`):**
   - Verify `ACTION_CATALOG.len() == 27`.
   - Verify `ViewAndSettings` count is 5.
   - Verify default accelerators resolve correctly for zoom actions (`<Primary>plus`, `<Primary>minus`, `<Primary>0`).
   - Verify full accelerator lists include keypad aliases without collision.
2. **TerminalPane Zoom Unit Tests (`src/ui/terminal_pane.rs`):**
   - Verify initial `font_scale() == 1.0`.
   - Verify `zoom_in()` increases scale by `0.1` to `1.1`.
   - Verify repeated zoom in clamps at `ZOOM_MAX` (`5.0`).
   - Verify repeated zoom out clamps at `ZOOM_MIN` (`0.2`).
   - Verify `zoom_normal()` resets scale to `1.0`.
3. **SessionView Active Dispatch Tests (`src/ui/session_view.rs`):**
   - In a split session with two panes, verify `zoom_in_active()` only alters the active pane's `font_scale()`, leaving the inactive pane at `1.0`.
4. **Window Action Registration Tests (`src/ui/window.rs`):**
   - Verify `win.zoom-in`, `win.zoom-out`, and `win.zoom-normal` actions exist on `TilixWindow`.
5. **Phase 13 Integration Test Suite (`tests/test_phase13_zoom_and_alt_drag.rs`):**
   - Test zoom steps, precision rounding, and clamping.
   - Test `EventControllerScroll` capture phase and modifier filtering.
   - Test `setup_pane_drag_source` with `require_alt = true` and `false`.
   - Test gesture denial when Alt is not pressed.

### 6.2 Validation Commands
```bash
# Targeted catalog tests
cargo test --test test_phase7_keybindings

# Targeted Phase 13 integration tests
cargo test --test test_phase13_zoom_and_alt_drag

# Full workspace regression
cargo test
```

---

## 7. Residual Risks & Tech Debt

1. **Floating-point Display in Future Preferences:**
   If font scale is ever rendered in a UI label, rounding to 1 decimal place (`((scale * 10.0).round() / 10.0)`) prevents values like `1.1000000000000002`. This guarantee is enforced by design in `TerminalPane`.
2. **Keypad Key Identification:**
   Some Wayland compositors may send distinct keysyms for keypad keys (`KP_Add`, `KP_Subtract`, `KP_0`). Including both main keyboard keys and keypad keys in `default_accels` provides out-of-the-box compatibility across Wayland and X11 environments.
