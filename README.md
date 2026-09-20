# Tilix (Rust Rewrite)

[![License: MPL-2.0](https://img.shields.io/badge/License-MPL_2.0-blue.svg)](https://opensource.org/licenses/MPL-2.0)
[![Language: Rust](https://img.shields.io/badge/Language-Rust_2021-orange.svg)](https://www.rust-lang.org/)
[![Toolkit: GTK4 & Libadwaita](https://img.shields.io/badge/GUI-GTK4_%2F_Libadwaita-green.svg)](https://gtk.org/)
[![Distribution: Arch Linux Native](https://img.shields.io/badge/Packaging-Arch_Linux-1793d1.svg)](https://archlinux.org/)

**Tilix** is an advanced tiling terminal emulator designed for Linux, adhering to GNOME Human Interface Guidelines (HIG).

This project is a modern, clean-room reimplementation of the original Tilix in **safe Rust**, using **GTK4**, **Libadwaita**, and **VTE4**.

---

## Background & Motivation

The original [Tilix](https://github.com/gnunn1/tilix) was written in the D programming language using GTK3. Over time, the upstream project became unmaintained, and D compiler packages were phased out of official Arch Linux repositories, making standard installation (`pacman -S tilix`) impossible.

This Rust rewrite restores Tilix with complete feature parity and modern desktop enhancements:
- **Zero Legacy Toolchain Dependencies:** Pure Rust codebase compiled with standard `cargo` and `rustc`.
- **Modern Desktop Stack:** GTK4, Libadwaita, and VTE4 with native Wayland support and clean dark/light styling.
- **Decoupled Headless Domain:** Unlike the original where the GTK widget hierarchy was the application state, this rewrite features a pure binary split tree (`LayoutTree`) that is 100% unit-testable in headless environments.
- **Drop-in Arch Linux Replacement:** Native `PKGBUILD` providing and conflicting with legacy `tilix`.

---

## Key Features

- **Binary Tree Tiling:** Split terminal panes horizontally (`Ctrl+Shift+D`) or vertically (`Ctrl+Shift+R`) with draggable dividers, auto-balancing (`Ctrl+Shift+B`), and 2D ray-cast directional navigation (`Alt+Arrow`).
- **Multi-Session Tab Management:** Organize workspaces using `AdwTabView` and `AdwTabBar` with automatic tab-bar autohide, tab reordering, and cross-window tab detachment.
- **Synchronized Keystroke Broadcast:** Broadcast terminal input across all split panes in a session with a cycle-free commit architecture (`Ctrl+Shift+I`), plus per-pane sync overrides.
- **Wayland Quake Drop-Down Mode:** Top-docked drop-down terminal (`tilix --quake-toggle`) designed for Wayland compositors via single-instance D-Bus IPC.
- **OSC 7 Working Directory Inheritance:** Automatically inherits the active shell's current working directory when splitting panes or opening new tabs.
- **Desktop Notifications:** Dispatches desktop notifications via `GNotification` when terminal bells or background processes exit while unfocused.
- **Drag-and-Drop Pane Reorganization:** Drag pane headers to swap terminal positions interactively.
- **Interactive Preferences Window:** Dedicated `AdwPreferencesWindow` (`Ctrl+,`) with color scheme selection (Tilix Dark, Tilix Light, Solarized, Monokai), cursor styling, and font settings.
- **Layout Templates:** Export and import session layout arrangements to JSON.

---

## Keyboard Shortcuts

| Shortcut | Action | Description |
| :--- | :--- | :--- |
| `Ctrl + Shift + R` | Split Right | Split active terminal vertically into two panes |
| `Ctrl + Shift + D` | Split Down | Split active terminal horizontally into two panes |
| `Ctrl + Shift + W` | Close Pane | Close active pane (or close session tab if last pane) |
| `Ctrl + Shift + B` | Balance Layout | Auto-balance split ratios equally |
| `Alt + Up/Down/Left/Right` | Directional Focus | Navigate focus across adjacent panes |
| `Ctrl + Shift + T` | New Tab | Open a new session tab (inheriting working directory) |
| `Ctrl + PageDown` / `Ctrl + PageUp` | Next / Previous Tab | Cycle through active session tabs |
| `Alt + 1` .. `Alt + 9` | Switch Tab | Jump directly to tab index 1 through 9 |
| `Ctrl + Shift + I` | Toggle Sync Input | Enable/disable synchronized keyboard broadcast |
| `Ctrl + ,` | Preferences | Open application preferences dialog |

---

## Command-Line Usage

Tilix supports single-instance activation over D-Bus:

```bash
# Launch normal application window
tilix

# Toggle visibility of the Wayland Quake drop-down window
tilix --quake-toggle

# Open Quake drop-down terminal directly
tilix --quake

# Open Preferences window
tilix --preferences

# Show command-line help
tilix --help

# Show version information
tilix --version
```

> **Tip (Wayland Quake Hotkey):** In your Wayland compositor (GNOME Shell, Sway, Hyprland, etc.), bind a global shortcut (e.g. `F12`) to `tilix --quake-toggle`.

---

## Installation & Packaging

### Dependencies

- `rust` & `cargo` (1.80+)
- `gtk4` (>= 4.18)
- `libadwaita` (>= 1.6)
- `vte4` / `vte-2.91-gtk4` (>= 0.76)

### Method 1: Arch Linux Native Package (`PKGBUILD`)

The included `PKGBUILD` builds and packages Tilix directly from the local source tree with full FreeDesktop integration:

```bash
# Build and install the Arch package from the repository root
makepkg -si
```

*The package provides and conflicts with `tilix`, serving as a seamless drop-in replacement for `pacman`.*

### Method 2: Standard Makefile Installation

```bash
# Build optimized release binary
make build

# Install binary, desktop entry, metainfo, icons, and shell completions
sudo make install PREFIX=/usr

# To uninstall:
sudo make uninstall PREFIX=/usr
```

### Method 3: Flatpak Sandbox

A Flatpak manifest targeting GNOME Runtime 47 is available at `build-aux/com.github.tilix_rust.json`:

```bash
flatpak-builder --user --install --force-clean build-dir build-aux/com.github.tilix_rust.json
```

---

## Development & Testing

Tilix's domain logic (binary tree splitting, ray-cast navigation, ratio scaling, ID remapping, CWD parsing, and argument routing) is completely decoupled from GTK and can be verified headless in CI without a display server:

```bash
# Run the entire test suite (179 tests)
cargo test

# Run linter checks
cargo clippy --all-targets -- -D warnings

# Validate desktop metadata
desktop-file-validate data/com.github.tilix_rust.desktop
appstreamcli validate --no-net data/com.github.tilix_rust.metainfo.xml
```

---

## Documentation

- **Living Architecture:** [`docs/architecture/overview.md`](docs/architecture/overview.md)
- **Specifications:**
  - [`spec-0001: Foundation`](docs/specs/spec-0001-rust-tilix-foundation-2026-09-15.md)
  - [`spec-0002: Sessions, Input Sync & Profiles`](docs/specs/spec-0002-sessions-sync-profiles-2026-09-15.md)
  - [`spec-0003: Wayland Quake, OSC 7 & DND`](docs/specs/spec-0003-wayland-quake-osc-dnd-2026-09-15.md)
  - [`spec-0004: Packaging, Distribution & Polish`](docs/specs/spec-0004-packaging-distribution-polish-2026-09-16.md)
- **Delivery Results:**
  - [`result-0001`](docs/results/result-0001.md)
  - [`result-0002`](docs/results/result-0002.md)
  - [`result-0003`](docs/results/result-0003.md)
  - [`result-0004`](docs/results/result-0004.md)

---

## License

This project is licensed under the Mozilla Public License 2.0 ([MPL-2.0](LICENSE)).
