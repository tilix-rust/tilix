# Maintainer: Tilix Rust Team <tilix@example.com>
pkgname=tilix-rust
_pkgname=tilix
pkgver=0.15.6
pkgrel=1
pkgdesc="A tiling terminal emulator for Linux using GTK4, Libadwaita and Rust"
arch=('x86_64' 'aarch64')
url="https://github.com/tilix-rust/tilix"
license=('MPL-2.0')
depends=('gtk4' 'libadwaita' 'vte4')
makedepends=('cargo' 'rust')
provides=('tilix')
conflicts=('tilix')
# Isolate makepkg build/scratch directories so $srcdir does not collide with the project's src/ directory.
# This prevents `makepkg -C` (--cleanbuild) or `makepkg -c` (--clean) from deleting the Rust source tree.
if [[ -z "$BUILDDIR" || "$BUILDDIR" -ef "$startdir" ]]; then
    BUILDDIR="$startdir/target/makepkg"
fi

source=()
sha256sums=()

prepare() {
    cd "$startdir"
    if [ -f Cargo.toml ]; then
        cargo fetch --locked --target "$(rustc -vV | sed -n 's/host: //p')" 2>/dev/null || cargo fetch --target "$(rustc -vV | sed -n 's/host: //p')" 2>/dev/null || true
    fi
}

build() {
    cd "$startdir"
    export CARGO_TARGET_DIR=target
    cargo build --release --all-targets
}

check() {
    cd "$startdir"
    cargo test --release
}

package() {
    cd "$startdir"
    
    # Binary
    install -Dm755 "target/release/tilix" "${pkgdir}/usr/bin/tilix"
    ln -sf "tilix" "${pkgdir}/usr/bin/tilix-rust"

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
