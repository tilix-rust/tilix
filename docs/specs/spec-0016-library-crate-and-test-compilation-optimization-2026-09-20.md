# Master Specification: Phase 16 — Dual Library Crate & Test Compilation Optimization

- **Document ID:** `SPEC-0016`
- **Date:** 2026-09-20
- **Status:** Proposed / Ready for Execution
- **Workflow Level:** Medium/Large (Phase 16 of Tilix Rewrite)
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
  - [`docs/specs/spec-0013-terminal-zoom-and-alt-drag-2026-09-18.md`](file:///playground/tilix/docs/specs/spec-0013-terminal-zoom-and-alt-drag-2026-09-18.md)
  - [`docs/specs/spec-0014-osc52-and-clipboard-integration-2026-09-19.md`](file:///playground/tilix/docs/specs/spec-0014-osc52-and-clipboard-integration-2026-09-19.md)
  - [`docs/specs/spec-0015-compact-mode-density-2026-09-19.md`](file:///playground/tilix/docs/specs/spec-0015-compact-mode-density-2026-09-19.md)
- **Target Location:** `/playground/tilix`

---

## 1. Context & Motivation

### 1.1 The Root Cause: Binary-Only Architecture Debt
When the Tilix rewrite began in Phase 1 (`spec-0001`), the project was initialized as a standard standalone Rust binary crate with its root at `src/main.rs`. At that time, the codebase was small (<1,500 lines) with a single domain test file.

Because Rust binary crates cannot be imported as external libraries via `extern crate` or `use <crate_name>::...` by integration test targets located in `tests/`, integration test suites relied on Rust's `#[path = "../src/..."]` attribute to directly pull in application source files into the test compilation units:
```rust
#[path = "../src/model/mod.rs"]
mod model;
#[path = "../src/pty/mod.rs"]
mod pty;
#[path = "../src/ui/mod.rs"]
mod ui;
#[path = "../src/app.rs"]
mod app;
```

### 1.2 The Growth: Codebase and Test Suite Expansion
Across Phases 1 through 15, Tilix expanded into a full-featured terminal emulator:
- **Total Source Code:** **16,621 lines** of Rust across `src/model/`, `src/pty/`, `src/ui/`, and `src/app.rs`.
- **GTK4 / Libadwaita / VTE Footprint:** The UI subsystem (`src/ui/`) grew to over 10,000 lines, featuring extensive macro expansions, widget hierarchy bindings, signal closures, and generic instantiations.
- **Integration Test Suites:** Expanded to **14 distinct test binaries** (`tests/test_phase2_domain.rs` through `tests/test_phase15_compact_mode.rs`), encompassing 194 automated unit and integration tests.
- **Widespread Re-inclusion:** 10 of the 14 integration test suites include `src/ui/mod.rs` (along with `model`, `pty`, and `app`), while the remaining 4 include `model`.

### 1.3 The Problem: The Parallel Compilation RAM Spike & Build Thrashing
Because each test file in `tests/*.rs` is compiled by Cargo as an entirely independent binary executable root, every single integration test re-compiled the entire application source tree from scratch:
1. **Compilation Multiplicity:** Cargo invoked `rustc` on the complete 16.6k-line application codebase not once, but **15 separate times** (1 for `src/main.rs` + 14 for `tests/test_phase*.rs`).
2. **Parallel Scheduling RAM Spike:** By default, Cargo schedules build jobs concurrently to match CPU core counts (e.g., 8 to 16 jobs). During `cargo test` and particularly `cargo test --release` (mandated by Arch Linux `PKGBUILD` in `check()`):
   - 10 to 14 concurrent `rustc` instances each loaded GTK4, Libadwaita, and VTE metadata, monomorphized identical generics, ran release optimization passes (`opt-level = 3`), and linked against system libraries.
   - Each `rustc` process consumed **1.5 GB to 2.5+ GB** of resident set size (RSS).
   - Concurrently executing 8-14 instances generated an acute **RAM spike of 15 GB to 30+ GB**.
3. **Severe Environmental Failures:** On developer workstations, CI runners, and packaging environments with 8 GB to 16 GB of RAM, this compilation explosion caused:
   - Aggressive OS swap thrashing and CPU starvation.
   - Linux Out-Of-Memory (`oom-killer`) terminations of `rustc` processes.
   - Failures during native package construction via `makepkg -s` or `make test`.

### 1.4 The Solution: Option 2 — Dual Library + Binary Architecture
Refactoring the project into a dual **Library (`src/lib.rs`) + Binary (`src/main.rs`)** crate resolves this architectural bottleneck fundamentally:
- `src/lib.rs` compiles the entire 16,621 lines of Tilix core logic once into `libtilix.rlib`.
- `src/main.rs` becomes an ultra-thin 6-line launcher that links against `tilix`.
- All 14 integration tests become lightweight consumers that simply import `use tilix::{app, model, pty, ui};` and link against `libtilix.rlib`.
- **Result:** Peak compilation memory drops by ~85-90% (to <2.5 GB total), test build time drops drastically, and all non-idiomatic `#[path = ...]` hacks are completely eliminated.

---

## 2. Amended Documents & Scope

- **Amended Document:** [`docs/architecture/overview.md`](file:///playground/tilix/docs/architecture/overview.md)
- **Status of Living Architecture:** Active (Version bumped to 0.16.0 for Phase 16)

### 2.1 Affected Scope

1. **`Cargo.toml`:**
   - Define explicit `[lib]` target pointing to `src/lib.rs` with `name = "tilix"`.
   - Define explicit `[[bin]]` target pointing to `src/main.rs` with `name = "tilix"`.
2. **`src/lib.rs` (New File):**
   - Declare and expose public modules:
     ```rust
     pub mod app;
     pub mod model;
     pub mod pty;
     pub mod ui;
     ```
3. **`src/main.rs`:**
   - Remove internal module declarations (`pub mod app; pub mod model; pub mod pty; pub mod ui;`).
   - Import and invoke `tilix::app::TilixApplication`.
4. **Integration Test Suites (`tests/`):**
   - Migrate all 14 integration test files by replacing `#[path = "../src/..."]` module inclusions with idiomatic `use tilix::{...}` imports:
     - `tests/test_phase2_domain.rs`
     - `tests/test_phase3_domain.rs`
     - `tests/test_phase4_packaging_polish.rs`
     - `tests/test_phase5_window_style_wide_handle.rs`
     - `tests/test_phase6_pane_toolbar.rs`
     - `tests/test_phase7_keybindings.rs`
     - `tests/test_phase8_dnd.rs`
     - `tests/test_phase9_profile.rs`
     - `tests/test_phase10_profile_ui_parity.rs`
     - `tests/test_phase11_title_options.rs`
     - `tests/test_phase12_window_geometry.rs`
     - `tests/test_phase13_zoom_and_alt_drag.rs`
     - `tests/test_phase14_osc52_and_clipboard.rs`
     - `tests/test_phase15_compact_mode.rs`
5. **Living Architecture (`docs/architecture/overview.md`):**
   - Update architecture version to 0.16.0.
   - Add Section 28 detailing the Dual Library + Binary architecture, compilation dependency graph, and memory optimization outcomes.

### 2.2 Unaffected Scope

- **Domain Model Logic:** `src/model/*` (`config`, `layout`, `keybindings`, `profile`, `session`, `template`, `theme`, `title`) is completely untouched.
- **PTY & Shell Spawning:** `src/pty/*` (`osc52`, `proxy`, `shell`) is completely untouched.
- **UI Subsystem:** `src/ui/*` (`dnd`, `geometry`, `notifications`, `preferences`, `quake`, `session_view`, `terminal_pane`, `window`) is completely untouched.
- **Application Logic:** `src/app.rs` internal logic is completely untouched (all internal `crate::ui::...` paths continue to resolve seamlessly within the library crate root).
- **Packaging & Desktop Files:** `Makefile`, `PKGBUILD`, `data/com.github.tilix_rust.desktop`, `data/com.github.tilix_rust.metainfo.xml`, `data/com.github.tilix_rust.service`, `build-aux/com.github.tilix_rust.json`, and completion scripts remain unchanged and fully functional. The generated binary path remains `target/release/tilix`.

---

## 3. Goals & Non-Goals

### Goals

1. **Dual Library + Binary Target Architecture:**
   - Establish `src/lib.rs` as the primary compilation unit (`libtilix.rlib`) containing the entire domain, PTY, UI, and application logic.
   - Retain `src/main.rs` as the executable entrypoint compiling to binary executable `tilix`.
2. **Single Compilation of Application Code:**
   - Ensure the 16,621 lines of code in `src/` are compiled exactly once into `libtilix.rlib`.
   - Prevent any redundant compilation of `src/` by test binaries.
3. **Drastic Peak RAM & Build Time Reduction:**
   - Eliminate the 15–30+ GB parallel compilation memory spike.
   - Lower peak memory during `cargo test --release` to <2.5 GB total.
   - Cut warm/incremental test build times by 80–90%.
4. **Complete Elimination of Non-Idiomatic Imports:**
   - Remove every instance of `#[path = "../src/..."]` across all 14 integration test suites in `tests/`.
   - Use standard, clean `use tilix::{app, model, pty, ui};` imports.
5. **Zero Test Regressions:**
   - Maintain 100% test pass rate across all 194 unit and integration tests under `cargo test`.
6. **Packaging Binary Compatibility:**
   - Guarantee that `cargo build --release` produces binary `target/release/tilix`, preserving complete compatibility with `Makefile`, `PKGBUILD`, and Flatpak.

### Non-Goals

- **Multi-Crate Cargo Workspace:** Refactoring Tilix into a multi-package workspace (e.g. `crates/tilix-core`, `crates/tilix-ui`, `crates/tilix-app`) is explicitly out of scope. A single crate with both `[lib]` and `[[bin]]` achieves the exact same compilation deduplication without workspace manifest overhead, path dependency friction, or circular dependency risks.
- **API Visibility Churn:** No unnecessary changes to module privacy. All modules `app`, `model`, `pty`, and `ui` are declared `pub mod` at the library root, matching the previous `src/main.rs` declarations.
- **Modifying Test Logic:** No changes to test assertions, fixtures, or coverage. Only the import headers are modified.

---

## 4. Architecture & Design Decisions

### 4.1 Decision Gating: Facts, Decisions, Assumptions, Deferred Items

- **Facts:**
  - Cargo natively supports a dual library and binary crate in the same package when `src/lib.rs` and `src/main.rs` are present.
  - In Rust, integration tests in `tests/*.rs` are separate crates that automatically receive the package's library crate as a dependency accessible via `tilix::...`.
  - Binary targets (`[[bin]]`) in a dual crate automatically link against the package's library target.
  - Internal module references inside `src/` (such as `use crate::ui::window::...` inside `src/app.rs`) refer to the library crate root when compiled as part of `src/lib.rs`.
- **Decisions:**
  - **Option 2 Architecture (Dual Library + Binary):** Explicitly declare `[lib]` and `[[bin]]` in `Cargo.toml`. This makes target resolution explicit and immune to variations across Cargo versions.
  - **Transparent Module Exposure:** `src/lib.rs` exports `pub mod app; pub mod model; pub mod pty; pub mod ui;`, exactly mirroring the original `src/main.rs` structure so that existing integration test paths (`model::layout::...`, `ui::session_view::...`, `app::parse_cli_args`, etc.) require zero semantic changes.
  - **Explicit Target Names:** Set `[lib] name = "tilix"` and `[[bin]] name = "tilix"`. Cargo compiles the binary executable to `target/{debug,release}/tilix` and the library to `target/{debug,release}/libtilix.rlib` without naming collision.
- **Assumptions:**
  - Standard Linux linkers (`ld`, `gold`, `lld`, or `mold`) handle linking test binaries against `libtilix.rlib` without requiring additional compiler flags.
- **Deferred Items:**
  - Future internal crate splitting (e.g., separating headless `model` into an independent published crate) is deferred until Tilix reaches 1.0 or requires external third-party consumer integration.

### 4.2 Architecture Decision Table

| Option | Pros | Cons | Verdict |
| :--- | :--- | :--- | :--- |
| **Option 1: Status Quo (`#[path = "..."]` in Binary Crate)** | No changes to `Cargo.toml` or module structure. | 14x duplicated compilation of 16.6k lines; 15-30+ GB RAM spikes; OOM failures in CI and `makepkg`; slow builds. | **Rejected** |
| **Option 2: Dual Library + Binary Crate (Chosen)** | Idiomatic Cargo architecture; single compilation of `src/` into `libtilix.rlib`; ~90% RAM reduction; 80-90% faster test builds; zero changes to internal `crate::` paths; preserves `target/release/tilix` binary path. | Requires updating import headers across 14 test files and configuring `Cargo.toml`. | **Accepted** |
| **Option 3: Multi-Crate Cargo Workspace** | Clean separation of domain and UI layers into independent crates. | High churn; requires multiple `Cargo.toml` files; complex inter-crate versioning; circular dependency risk between UI and App; overkill for single desktop application. | **Rejected** |

---

## 5. Detailed Technical Specifications

### 5.1 Cargo Manifest Configuration (`Cargo.toml`)

Update `Cargo.toml` to explicitly define the library and binary targets:

```toml
[package]
name = "tilix"
version = "0.15.6"
edition = "2021"

[lib]
name = "tilix"
path = "src/lib.rs"

[[bin]]
name = "tilix"
path = "src/main.rs"

[dependencies]
gtk4 = { version = "0.11", features = ["v4_18"] }
libadwaita = { version = "0.9", features = ["v1_6"] }
vte4 = { version = "0.10", features = ["v0_76"] }
glib = "0.22"
gio = "0.22"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "2.0"
libc = "0.2"
```

### 5.2 Library Root (`src/lib.rs`)

Create `src/lib.rs` to expose the core application modules:

```rust
pub mod app;
pub mod model;
pub mod pty;
pub mod ui;
```

When Cargo compiles `tilix` as a library:
1. `src/model/` is compiled as `tilix::model`.
2. `src/pty/` is compiled as `tilix::pty`.
3. `src/ui/` is compiled as `tilix::ui`.
4. `src/app.rs` is compiled as `tilix::app`.
All internal references within `src/` using `crate::...` (e.g. `crate::ui::window::setup_css()` in `src/app.rs`) continue to refer to the crate root of `libtilix.rlib`.

### 5.3 Binary Entrypoint (`src/main.rs`)

Refactor `src/main.rs` to become a lean binary entrypoint:

```rust
fn main() -> glib::ExitCode {
    let app = tilix::app::TilixApplication::new();
    let args: Vec<String> = std::env::args().collect();
    app.run_with_args(&args)
}
```

The binary depends on `tilix` (the library) and simply launches `TilixApplication`. Compiling `src/main.rs` takes less than 0.2 seconds because it does not recompile any application code.

### 5.4 Migration of Integration Test Suites (`tests/`)

All 14 integration test suites in `tests/` are migrated from `#[path = "..."]` inclusions to standard `use tilix::...` imports.

#### 1. `tests/test_phase2_domain.rs`
Replace:
```rust
#[path = "../src/model/mod.rs"]
mod model;
```
With:
```rust
use tilix::model;
```

#### 2. `tests/test_phase3_domain.rs`
Replace:
```rust
#[path = "../src/model/mod.rs"]
mod model;

#[path = "../src/pty/mod.rs"]
mod pty;

#[path = "../src/ui/mod.rs"]
mod ui;

#[path = "../src/app.rs"]
mod app;
```
With:
```rust
use tilix::{app, model, pty, ui};
```

#### 3. `tests/test_phase4_packaging_polish.rs`
Replace:
```rust
#[path = "../src/model/mod.rs"]
mod model;

#[path = "../src/pty/mod.rs"]
mod pty;

#[path = "../src/ui/mod.rs"]
mod ui;

#[path = "../src/app.rs"]
mod app;
```
With:
```rust
use tilix::{app, model, pty, ui};
```

#### 4. `tests/test_phase5_window_style_wide_handle.rs`
Replace:
```rust
#[path = "../src/model/mod.rs"]
mod model;
```
With:
```rust
use tilix::model;
```

#### 5. `tests/test_phase6_pane_toolbar.rs`
Replace:
```rust
#[path = "../src/model/mod.rs"]
mod model;
```
With:
```rust
use tilix::model;
```

#### 6. `tests/test_phase7_keybindings.rs`
Replace:
```rust
#[path = "../src/model/mod.rs"]
mod model;
```
With:
```rust
use tilix::model;
```

#### 7. `tests/test_phase8_dnd.rs`
Replace:
```rust
#[path = "../src/model/mod.rs"]
pub mod model;

#[path = "../src/pty/mod.rs"]
pub mod pty;

#[path = "../src/ui/mod.rs"]
pub mod ui;

#[path = "../src/app.rs"]
pub mod app;
```
With:
```rust
use tilix::{app, model, pty, ui};
```

#### 8. `tests/test_phase9_profile.rs`
Replace:
```rust
#[path = "../src/model/mod.rs"]
pub mod model;

#[path = "../src/pty/mod.rs"]
pub mod pty;

#[path = "../src/ui/mod.rs"]
pub mod ui;

#[path = "../src/app.rs"]
pub mod app;
```
With:
```rust
use tilix::{app, model, pty, ui};
```

#### 9. `tests/test_phase10_profile_ui_parity.rs`
Replace:
```rust
#[path = "../src/model/mod.rs"]
pub mod model;

#[path = "../src/pty/mod.rs"]
pub mod pty;

#[path = "../src/ui/mod.rs"]
pub mod ui;

#[path = "../src/app.rs"]
pub mod app;
```
With:
```rust
use tilix::{app, model, pty, ui};
```

#### 10. `tests/test_phase11_title_options.rs`
Replace:
```rust
#[path = "../src/model/mod.rs"]
pub mod model;

#[path = "../src/pty/mod.rs"]
pub mod pty;

#[path = "../src/ui/mod.rs"]
pub mod ui;

#[path = "../src/app.rs"]
pub mod app;
```
With:
```rust
use tilix::{app, model, pty, ui};
```

#### 11. `tests/test_phase12_window_geometry.rs`
Replace:
```rust
#[path = "../src/model/mod.rs"]
pub mod model;

#[path = "../src/pty/mod.rs"]
pub mod pty;

#[path = "../src/ui/mod.rs"]
pub mod ui;

#[path = "../src/app.rs"]
pub mod app;
```
With:
```rust
use tilix::{app, model, pty, ui};
```

#### 12. `tests/test_phase13_zoom_and_alt_drag.rs`
Replace:
```rust
#[path = "../src/model/mod.rs"]
pub mod model;

#[path = "../src/pty/mod.rs"]
pub mod pty;

#[path = "../src/ui/mod.rs"]
pub mod ui;

#[path = "../src/app.rs"]
pub mod app;
```
With:
```rust
use tilix::{app, model, pty, ui};
```

#### 13. `tests/test_phase14_osc52_and_clipboard.rs`
Replace:
```rust
#[path = "../src/model/mod.rs"]
pub mod model;

#[path = "../src/pty/mod.rs"]
pub mod pty;

#[path = "../src/ui/mod.rs"]
pub mod ui;

#[path = "../src/app.rs"]
pub mod app;
```
With:
```rust
use tilix::{app, model, pty, ui};
```

#### 14. `tests/test_phase15_compact_mode.rs`
Replace:
```rust
#[path = "../src/model/mod.rs"]
pub mod model;

#[path = "../src/pty/mod.rs"]
pub mod pty;

#[path = "../src/ui/mod.rs"]
pub mod ui;

#[path = "../src/app.rs"]
pub mod app;
```
With:
```rust
use tilix::{app, model, pty, ui};
```

---

## 6. File Manifest & Detailed Code Modifications

| File | Modification Type | Description of Changes |
| :--- | :--- | :--- |
| `Cargo.toml` | Modified | Add `[lib]` and `[[bin]]` sections pointing to `src/lib.rs` and `src/main.rs`. |
| `src/lib.rs` | Created | Expose `pub mod app; pub mod model; pub mod pty; pub mod ui;`. |
| `src/main.rs` | Modified | Strip internal module declarations; call `tilix::app::TilixApplication::new()`. |
| `tests/test_phase2_domain.rs` | Modified | Replace `#[path = ...]` with `use tilix::model;`. |
| `tests/test_phase3_domain.rs` | Modified | Replace `#[path = ...]` with `use tilix::{app, model, pty, ui};`. |
| `tests/test_phase4_packaging_polish.rs` | Modified | Replace `#[path = ...]` with `use tilix::{app, model, pty, ui};`. |
| `tests/test_phase5_window_style_wide_handle.rs` | Modified | Replace `#[path = ...]` with `use tilix::model;`. |
| `tests/test_phase6_pane_toolbar.rs` | Modified | Replace `#[path = ...]` with `use tilix::model;`. |
| `tests/test_phase7_keybindings.rs` | Modified | Replace `#[path = ...]` with `use tilix::model;`. |
| `tests/test_phase8_dnd.rs` | Modified | Replace `#[path = ...]` with `use tilix::{app, model, pty, ui};`. |
| `tests/test_phase9_profile.rs` | Modified | Replace `#[path = ...]` with `use tilix::{app, model, pty, ui};`. |
| `tests/test_phase10_profile_ui_parity.rs` | Modified | Replace `#[path = ...]` with `use tilix::{app, model, pty, ui};`. |
| `tests/test_phase11_title_options.rs` | Modified | Replace `#[path = ...]` with `use tilix::{app, model, pty, ui};`. |
| `tests/test_phase12_window_geometry.rs` | Modified | Replace `#[path = ...]` with `use tilix::{app, model, pty, ui};`. |
| `tests/test_phase13_zoom_and_alt_drag.rs` | Modified | Replace `#[path = ...]` with `use tilix::{app, model, pty, ui};`. |
| `tests/test_phase14_osc52_and_clipboard.rs` | Modified | Replace `#[path = ...]` with `use tilix::{app, model, pty, ui};`. |
| `tests/test_phase15_compact_mode.rs` | Modified | Replace `#[path = ...]` with `use tilix::{app, model, pty, ui};`. |
| `docs/architecture/overview.md` | Modified | Bump version to 0.16.0; add Section 28 detailing Dual Library + Binary Architecture. |
| `docs/specs/spec-0016-library-crate-and-test-compilation-optimization-2026-09-20.md` | Created | Master architectural specification for Phase 16. |
| `docs/tasks/task-0016.md` | Created | Executable step-by-step task checklist. |

---

## 7. Verification & Automated Test Strategy

### 7.1 Target Compilation Verification
Verify that both library, binary, and test targets build cleanly:
1. `cargo check --lib`: Verifies `src/lib.rs` and all 16.6k lines compile as `libtilix.rlib`.
2. `cargo check --bin tilix`: Verifies `src/main.rs` compiles and links against `tilix`.
3. `cargo check --tests`: Verifies all 14 test suites compile against `libtilix.rlib`.

### 7.2 Full Regression Suite
Run the complete automated test suite:
```bash
cargo test
```
Verify that all 194 unit and integration tests pass with 0 failures, 0 errors, and 0 warnings.

### 7.3 Packaging & Distribution Integrity
Validate that the packaging test suite passes without modification to packaging specifications:
```bash
cargo test --test test_phase4_packaging_polish
```
Assert that:
- `test_desktop_file_actions_and_cli_consistency` passes using `tilix::app::parse_cli_args`.
- `test_pkgbuild_content_specifications` passes.
- `test_makefile_content_specifications` passes.
- `test_flatpak_manifest_validity` passes.

### 7.4 Release Compilation & Peak Memory Validation
Verify release build efficiency:
```bash
cargo test --release --no-run
```
Assert that:
- `libtilix.rlib` is compiled once in release mode.
- Subsequent test binary links finish rapidly without RAM explosion.
- Peak memory usage remains under 2.5 GB.

---

## 8. Residual Risks, Safety Guarantees & Tech Debt

| Risk | Impact | Mitigation Strategy |
| :--- | :--- | :--- |
| **Name Collision Between Lib and Bin** | Cargo might error if library and binary share the same name without proper target configuration. | `Cargo.toml` explicitly configures `[lib] name = "tilix"` and `[[bin]] name = "tilix"`. Cargo natively supports identical package, lib, and bin names by generating `libtilix.rlib` and `tilix` executable. |
| **Internal `crate::` Import Breakage** | Moving code to a library might break internal imports if paths assumed binary root. | All internal code in `src/` already used `crate::...` (e.g., `crate::ui::...`, `crate::model::...`), which resolves identically to `src/lib.rs` root. |
| **Packaging Artifact Missing Target** | Downstream packaging scripts might expect `target/release/tilix`. | The binary target `[[bin]] name = "tilix"` outputs to the exact same path `target/release/tilix`, guaranteeing 100% compatibility with `Makefile` and `PKGBUILD`. |
| **Integration Test Module Name Shadows** | `use tilix::model;` might shadow local names. | All existing tests already used `model::...`, `ui::...`, `pty::...`, or `app::...`. Replacing `#[path = "..."] mod model;` with `use tilix::model;` introduces the exact same symbol into scope. |

---

## 9. Rollout & Milestone Plan

- **Phase 16.1:** Configure `Cargo.toml`, create `src/lib.rs`, and refactor `src/main.rs`. Verify library and binary compilation.
- **Phase 16.2:** Migrate Phase 2 through Phase 7 integration tests (`test_phase2_domain.rs` to `test_phase7_keybindings.rs`).
- **Phase 16.3:** Migrate Phase 8 through Phase 11 integration tests (`test_phase8_dnd.rs` to `test_phase11_title_options.rs`).
- **Phase 16.4:** Migrate Phase 12 through Phase 15 integration tests (`test_phase12_window_geometry.rs` to `test_phase15_compact_mode.rs`).
- **Phase 16.5:** Verify packaging suite (`test_phase4_packaging_polish.rs`), test release compilation (`cargo test --release --no-run`), and execute full regression test suite (`cargo test`).
- **Phase 16.6:** Update Living Architecture (`docs/architecture/overview.md`) to version 0.16.0 with Section 28.
