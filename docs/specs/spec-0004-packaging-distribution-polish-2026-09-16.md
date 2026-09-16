# Master Specification: Phase 4 — Packaging, Desktop Integration, Distribution & Polish

- **Document ID:** `SPEC-0004`
- **Date:** 2026-09-16
- **Status:** Approved / Decision Ready
- **Workflow Level:** Medium/Large (Phase 4 of Tilix Rewrite)
- **Preceding Specification:** `docs/specs/spec-0003-wayland-quake-osc-dnd-2026-09-15.md`
- **Target Location:** `/playground/tilix`

---

## 1. Context & Motivation

The original Tilix tiling terminal emulator was written in the D programming language. Due to the deprecation and removal of D compilers and associated packages from official Arch Linux repositories, Tilix was dropped from Arch Linux, breaking `pacman -S tilix` for thousands of Linux users.

Across Phases 1, 2, and 3, the project was rewritten in modern Rust using **GTK4**, **Libadwaita**, and **VTE4**, establishing:
- A pure headless layout tree and session model.
- Multi-session tab management (`AdwTabView`/`AdwTabBar`) and input synchronization across splits.
- Wayland-native Quake drop-down window mode via single-instance D-Bus IPC.
- OSC 7 working directory inheritance, desktop notifications, tab detaching, and interactive preferences.

Phase 4 concludes the rewrite by delivering end-to-end packaging, system integration, distribution assets, and resolving final UI polish items. This equips Arch Linux users, Flatpak distributors, and general Linux environments with native install automation and desktop integration, restoring Tilix as a first-class Linux desktop application.

---

## 2. Goals & Non-Goals

### Goals
1. **Arch Linux Packaging (`PKGBUILD`):** Provide an Arch Linux package definition adhering to official Arch Rust packaging guidelines, declaring dependencies (`gtk4`, `libadwaita`, `vte4`), makedepends (`cargo`, `rust`), and clean `provides=('tilix')` and `conflicts=('tilix')`.
2. **FreeDesktop Desktop Entry (`data/com.github.tilix_rust.desktop`):** Provide a fully compliant desktop specification entry with Actions for "New Window", "Toggle Quake Window", and "Preferences", along with keywords and terminal categories.
3. **AppStream Metainfo (`data/com.github.tilix_rust.metainfo.xml`):** Provide valid AppStream metadata specifying application descriptions, features, release details, launchable IDs, and content ratings.
4. **D-Bus Service Activation (`data/com.github.tilix_rust.service`):** Provide a D-Bus session service file enabling D-Bus activation of `com.github.tilix_rust`.
5. **Vector Iconography:** Provide a scalable SVG application icon (`data/icons/scalable/apps/com.github.tilix_rust.svg`) adhering to GNOME Human Interface Guidelines.
6. **Shell Completions:** Provide completion scripts for `bash`, `zsh`, and `fish` covering all CLI flags (`--quake`, `--quake-toggle`, `--preferences`, `-p`, `-h`, `--help`, `-v`, `--version`).
7. **Flatpak Sandbox Manifest (`build-aux/com.github.tilix_rust.json`):** Provide a Flatpak build manifest targeting GNOME Platform 47 with appropriate sandbox permissions for terminal shell execution.
8. **Automated Installation (`Makefile`):** Provide standard build and install targets (`all`, `build`, `install`, `uninstall`, `clean`, `check`) honoring `DESTDIR` and `PREFIX`.
9. **Final UI & Model Polish:**
   - Wire action handlers in `TilixQuakeWindow` (`src/ui/quake.rs`) so that pane header buttons (Split Right, Split Down, Close) function properly.
   - Dynamically look up pane titles at notification event time in `src/ui/session_view.rs`.
   - Add `#[serde(default)]` to `AppConfig` in `src/model/config.rs` for forward and backward compatibility when parsing configuration files.

### Non-Goals
- Publishing to upstream AUR or Flathub git repositories (external network and SSH credential gating).
- Supporting legacy non-systemd init systems in packaging definitions.

---

## 3. Functional Requirements

### FR-1: Arch Linux Packaging Specification (`PKGBUILD`)
- Package name: `tilix-rust` (or `tilix`).
- Package version: `0.1.0`, release: `1`.
- Architecture: `x86_64`, `aarch64`.
- License: `MPL-2.0`.
- Dependencies: `gtk4`, `libadwaita`, `vte4`.
- Build dependencies: `cargo`, `rust`.
- Package conflict/replace: `provides=('tilix')`, `conflicts=('tilix')`.
- The `build()` step executes `cargo build --frozen --release --all-targets`.
- The `check()` step executes `cargo test --frozen --release`.
- The `package()` step installs:
  - Binary to `${pkgdir}/usr/bin/tilix` (with symlink `${pkgdir}/usr/bin/tilix-rust`).
  - Desktop file to `${pkgdir}/usr/share/applications/com.github.tilix_rust.desktop`.
  - AppStream metainfo to `${pkgdir}/usr/share/metainfo/com.github.tilix_rust.metainfo.xml`.
  - D-Bus service file to `${pkgdir}/usr/share/dbus-1/services/com.github.tilix_rust.service`.
  - Scalable icon to `${pkgdir}/usr/share/icons/hicolor/scalable/apps/com.github.tilix_rust.svg`.
  - Bash completion to `${pkgdir}/usr/share/bash-completion/completions/tilix`.
  - Zsh completion to `${pkgdir}/usr/share/zsh/site-functions/_tilix`.
  - Fish completion to `${pkgdir}/usr/share/fish/vendor_completions.d/tilix.fish`.

### FR-2: Desktop Entry Specification
- File: `data/com.github.tilix_rust.desktop`.
- Validated via `desktop-file-validate`.
- Must contain:
  - `Type=Application`
  - `Name=Tilix`
  - `GenericName=Terminal Emulator`
  - `Comment=A tiling terminal emulator for Linux using GTK4`
  - `Icon=com.github.tilix_rust`
  - `Exec=tilix`
  - `Terminal=false`
  - `Categories=GNOME;GTK;System;TerminalEmulator;`
  - `StartupNotify=true`
  - `StartupWMClass=com.github.tilix_rust`
  - `Actions=NewWindow;Quake;Preferences;`
  - `[Desktop Action NewWindow]` with `Exec=tilix`.
  - `[Desktop Action Quake]` with `Exec=tilix --quake-toggle`.
  - `[Desktop Action Preferences]` with `Exec=tilix --preferences`.

### FR-3: AppStream Metadata Specification
- File: `data/com.github.tilix_rust.metainfo.xml`.
- Validated via `appstreamcli validate --no-net`.
- Must contain component type `desktop-application`, ID `com.github.tilix_rust`, project name, summary, feature description, launchable desktop ID, licenses (`CC0-1.0` metadata, `MPL-2.0` project), developer ID, and release entry for `0.1.0`.

### FR-4: D-Bus Service Activation Specification
- File: `data/com.github.tilix_rust.service`.
- Must define `Name=com.github.tilix_rust` and `Exec=/usr/bin/tilix --gapplication-service`.

### FR-5: Shell Completion Specifications
- Provide complete completion scripts in `data/completions/`:
  - `bash/tilix`: Defines `_tilix()` completion handling `-h`, `--help`, `-v`, `--version`, `-p`, `--preferences`, `--quake`, and `--quake-toggle`.
  - `zsh/_tilix`: Defines `#compdef tilix` using `_arguments`.
  - `fish/tilix.fish`: Defines completions using `complete -c tilix`.

### FR-6: Flatpak Manifest Specification
- File: `build-aux/com.github.tilix_rust.json`.
- Targets `org.gnome.Platform` and `org.gnome.Sdk` version `47` with `org.freedesktop.Sdk.Extension.rust-stable`.
- Declares sandbox permissions: `--share=ipc`, `--socket=fallback-x11`, `--socket=wayland`, `--filesystem=host`, `--device=all`, `--talk-name=org.freedesktop.Notifications`.
- Builds binary and installs desktop file, metainfo, and icon into `/app`.

### FR-7: Makefile Install Automation
- File: `Makefile`.
- Standard targets: `all`, `build`, `install`, `uninstall`, `clean`, `test`.
- Configurable variables: `DESTDIR`, `PREFIX` (default `/usr/local`), `BINDIR`, `DATADIR`.
- Installs all assets matching standard Linux directory hierarchies.

### FR-8: UI Polish & Compatibility Fixes
1. **Quake Action Handler (`src/ui/quake.rs`):**
   - Connect `session_view.borrow().set_action_handler` inside `TilixQuakeWindow::new`.
   - Dispatch `SessionAction::Split` to `session.borrow_mut().split_active(orientation)`.
   - Dispatch `SessionAction::Close` to `session.borrow_mut().close_pane(id)`. If the session becomes empty, hide the Quake window and recreate an initial pane.
   - Dispatch `SessionAction::Focus` to `session.borrow_mut().set_active_pane(id)`.
2. **Dynamic Notification Title Query (`src/ui/session_view.rs`):**
   - In `create_pane()`, do not capture static strings `title_bell` and `title_exit`.
   - In the closures for `pane.connect_bell` and `pane.connect_child_exited`, query the current title via `panes.borrow().get(&p_id).map(|p| p.title())`.
3. **`AppConfig` Serde Defaults (`src/model/config.rs`):**
   - Add `#[serde(default)]` attribute to `AppConfig`.
   - Verify that missing fields in JSON deserialize to `AppConfig::default()` values.

---

## 4. Non-Functional Requirements

### NFR-1: Arch Linux Packaging Compliance
- The `PKGBUILD` must strictly adhere to Arch Linux RFC and Rust packaging guidelines (using `--frozen`, `--release`, standard path macros).
- `makepkg --printsrcinfo` must generate valid `.SRCINFO` without syntax errors.

### NFR-2: Zero-Warning Builds & Headless Testability
- All code additions and modifications must compile cleanly with `cargo check --all-targets` and pass `cargo clippy --all-targets -- -D warnings`.
- `cargo test` must run 100% headlessly in CI without requiring an active X11/Wayland display server.

### NFR-3: FreeDesktop Specification Compliance
- The desktop file must pass `desktop-file-validate` without warnings or errors.
- The AppStream metainfo file must pass `appstreamcli validate --no-net` without errors.

---

## 5. Detailed File Specifications & Manifest

### 5.1 `PKGBUILD`
```bash
# Maintainer: Tilix Rust Team <tilix@example.com>
pkgname=tilix-rust
_pkgname=tilix
pkgver=0.1.0
pkgrel=1
pkgdesc="A tiling terminal emulator for Linux using GTK4, Libadwaita and Rust"
arch=('x86_64' 'aarch64')
url="https://github.com/tilix-rust/tilix"
license=('MPL-2.0')
depends=('gtk4' 'libadwaita' 'vte4')
makedepends=('cargo' 'rust')
provides=('tilix')
conflicts=('tilix')
source=("$pkgname-$pkgver::git+https://github.com/tilix-rust/tilix.git#tag=v$pkgver")
sha256sums=('SKIP')

prepare() {
    cd "$srcdir/$_pkgname" 2>/dev/null || cd "$srcdir" 2>/dev/null || true
    if [ -f Cargo.toml ]; then
        cargo fetch --locked --target "$(rustc -vV | sed -n 's/host: //p')" || cargo fetch --target "$(rustc -vV | sed -n 's/host: //p')"
    fi
}

build() {
    cd "$srcdir/$_pkgname" 2>/dev/null || cd "$srcdir" 2>/dev/null || true
    export CARGO_TARGET_DIR=target
    cargo build --release --all-targets
}

check() {
    cd "$srcdir/$_pkgname" 2>/dev/null || cd "$srcdir" 2>/dev/null || true
    cargo test --release
}

package() {
    cd "$srcdir/$_pkgname" 2>/dev/null || cd "$srcdir" 2>/dev/null || true
    
    # Binary
    install -Dm755 "target/release/tilix" "${pkgdir}/usr/bin/tilix"
    ln -s "tilix" "${pkgdir}/usr/bin/tilix-rust"

    # Desktop Entry
    install -Dm644 "data/com.github.tilix_rust.desktop" "${pkgdir}/usr/share/applications/com.github.tilix_rust.desktop"

    # AppStream Metainfo
    install -Dm644 "data/com.github.tilix_rust.metainfo.xml" "${pkgdir}/usr/share/metainfo/com.github.tilix_rust.metainfo.xml"

    # D-Bus Service
    install -Dm644 "data/com.github.tilix_rust.service" "${pkgdir}/usr/share/dbus-1/services/com.github.tilix_rust.service"

    # Scalable Icon
    install -Dm644 "data/icons/scalable/apps/com.github.tilix_rust.svg" "${pkgdir}/usr/share/icons/hicolor/scalable/apps/com.github.tilix_rust.svg"

    # Shell Completions
    install -Dm644 "data/completions/bash/tilix" "${pkgdir}/usr/share/bash-completion/completions/tilix"
    install -Dm644 "data/completions/zsh/_tilix" "${pkgdir}/usr/share/zsh/site-functions/_tilix"
    install -Dm644 "data/completions/fish/tilix.fish" "${pkgdir}/usr/share/fish/vendor_completions.d/tilix.fish"
}
```

### 5.2 `data/com.github.tilix_rust.desktop`
```ini
[Desktop Entry]
Name=Tilix
Comment=A tiling terminal emulator for Linux using GTK4
GenericName=Terminal Emulator
Exec=tilix
Icon=com.github.tilix_rust
Terminal=false
Type=Application
Categories=GNOME;GTK;System;TerminalEmulator;
StartupNotify=true
StartupWMClass=com.github.tilix_rust
Keywords=terminal;prompt;command;commandline;tiling;quake;vte;
Actions=NewWindow;Quake;Preferences;

[Desktop Action NewWindow]
Name=New Window
Exec=tilix

[Desktop Action Quake]
Name=Toggle Quake Window
Exec=tilix --quake-toggle

[Desktop Action Preferences]
Name=Preferences
Exec=tilix --preferences
```

### 5.3 `data/com.github.tilix_rust.metainfo.xml`
```xml
<?xml version="1.0" encoding="UTF-8"?>
<component type="desktop-application">
  <id>com.github.tilix_rust</id>
  <metadata_license>CC0-1.0</metadata_license>
  <project_license>MPL-2.0</project_license>
  <name>Tilix</name>
  <summary>A modern tiling terminal emulator written in Rust and GTK4</summary>
  <description>
    <p>
      Tilix is an advanced GTK4 and Libadwaita tiling terminal emulator designed for Linux.
      Reimplemented in Rust for safety, reliability, and modern desktop compatibility, Tilix
      empowers developers and system administrators to arrange multiple terminal sessions in
      flexible splits, synchronize keystrokes across panes, and summon a top-docked Quake drop-down
      terminal seamlessly under Wayland.
    </p>
    <p>Key Features:</p>
    <ul>
      <li>Flexible horizontal and vertical tiling layouts with dynamic ratio adjustment.</li>
      <li>Multi-session tab support with automatic tab-bar autohide and cross-window detachment.</li>
      <li>Synchronized input broadcasting across terminal panes.</li>
      <li>Native Wayland Quake drop-down mode via single-instance D-Bus IPC.</li>
      <li>OSC 7 current working directory inheritance on splits and tabs.</li>
      <li>Interactive preferences with real-time palette and font reconfiguration.</li>
      <li>Native Arch Linux and Flatpak distribution with zero legacy D compiler dependencies.</li>
    </ul>
  </description>
  <launchable type="desktop-id">com.github.tilix_rust.desktop</launchable>
  <url type="homepage">https://github.com/tilix-rust/tilix</url>
  <url type="bugtracker">https://github.com/tilix-rust/tilix/issues</url>
  <provides>
    <binary>tilix</binary>
  </provides>
  <developer id="io.github.tilix_rust">
    <name>The Tilix Rust Project</name>
  </developer>
  <releases>
    <release version="0.1.0" date="2026-09-16">
      <description>
        <p>Initial stable release of the Rust Tilix rewrite featuring GTK4, Libadwaita, Wayland Quake mode, synchronized typing, and native packaging.</p>
      </description>
    </release>
  </releases>
  <content_rating type="oars-1.1"/>
</component>
```

### 5.4 `data/com.github.tilix_rust.service`
```ini
[D-BUS Service]
Name=com.github.tilix_rust
Exec=/usr/bin/tilix --gapplication-service
```

### 5.5 `Makefile`
```makefile
PREFIX ?= /usr/local
BINDIR ?= $(PREFIX)/bin
DATADIR ?= $(PREFIX)/share
APP_ID = com.github.tilix_rust

.PHONY: all build check test install uninstall clean

all: build

build:
	cargo build --release

check: test

test:
	cargo test

install: build
	install -d $(DESTDIR)$(BINDIR)
	install -m 755 target/release/tilix $(DESTDIR)$(BINDIR)/tilix
	install -d $(DESTDIR)$(DATADIR)/applications
	install -m 644 data/$(APP_ID).desktop $(DESTDIR)$(DATADIR)/applications/$(APP_ID).desktop
	install -d $(DESTDIR)$(DATADIR)/metainfo
	install -m 644 data/$(APP_ID).metainfo.xml $(DESTDIR)$(DATADIR)/metainfo/$(APP_ID).metainfo.xml
	install -d $(DESTDIR)$(DATADIR)/dbus-1/services
	install -m 644 data/$(APP_ID).service $(DESTDIR)$(DATADIR)/dbus-1/services/$(APP_ID).service
	install -d $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps
	install -m 644 data/icons/scalable/apps/$(APP_ID).svg $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps/$(APP_ID).svg
	install -d $(DESTDIR)$(DATADIR)/bash-completion/completions
	install -m 644 data/completions/bash/tilix $(DESTDIR)$(DATADIR)/bash-completion/completions/tilix
	install -d $(DESTDIR)$(DATADIR)/zsh/site-functions
	install -m 644 data/completions/zsh/_tilix $(DESTDIR)$(DATADIR)/zsh/site-functions/_tilix
	install -d $(DESTDIR)$(DATADIR)/fish/vendor_completions.d
	install -m 644 data/completions/fish/tilix.fish $(DESTDIR)$(DATADIR)/fish/vendor_completions.d/tilix.fish

uninstall:
	rm -f $(DESTDIR)$(BINDIR)/tilix
	rm -f $(DESTDIR)$(DATADIR)/applications/$(APP_ID).desktop
	rm -f $(DESTDIR)$(DATADIR)/metainfo/$(APP_ID).metainfo.xml
	rm -f $(DESTDIR)$(DATADIR)/dbus-1/services/$(APP_ID).service
	rm -f $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps/$(APP_ID).svg
	rm -f $(DESTDIR)$(DATADIR)/bash-completion/completions/tilix
	rm -f $(DESTDIR)$(DATADIR)/zsh/site-functions/_tilix
	rm -f $(DESTDIR)$(DATADIR)/fish/vendor_completions.d/tilix.fish

clean:
	cargo clean
```

---

## 6. Validation & Verification Strategy

1. **FreeDesktop Specification Validation:**
   - Run `desktop-file-validate data/com.github.tilix_rust.desktop` (must return 0).
   - Run `appstreamcli validate --no-net data/com.github.tilix_rust.metainfo.xml` (must return 0).
2. **Arch Linux Packaging Validation:**
   - Run `makepkg --printsrcinfo > .SRCINFO` to verify syntax.
3. **Automated Install/Uninstall Verification:**
   - Execute `make build`.
   - Execute `make install DESTDIR=/tmp/tilix-test-root PREFIX=/usr`.
   - Verify presence of binary, desktop file, metainfo, dbus service, svg icon, and all 3 shell completions under `/tmp/tilix-test-root`.
   - Execute `make uninstall DESTDIR=/tmp/tilix-test-root PREFIX=/usr`.
   - Verify complete cleanup.
4. **Rust Test Suite & Regressions:**
   - Execute `cargo test`.
   - Add unit tests in `tests/test_phase4_packaging_polish.rs` verifying `AppConfig` partial JSON deserialization and CLI completion tokens.
   - Execute `cargo clippy --all-targets -- -D warnings`.
