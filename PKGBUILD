# Maintainer: adamsolit
pkgname=soundcloud-desktop
pkgver=0.1.0
pkgrel=1
pkgdesc="Ultra-fast native GTK4/Adwaita SoundCloud desktop client for Arch Linux & Wayland"
arch=('x86_64')
url="https://github.com/adamsolit/soundcloud-in-rust"
license=('MIT')
depends=(
    'gtk4'
    'libadwaita'
    'mpv'
    'gstreamer'
    'gst-plugins-base'
    'gst-plugins-good'
    'openssl'
    'sqlite'
)
makedepends=('cargo' 'git')
source=()

build() {
    cargo build --release --locked
}

check() {
    cargo test --release --locked
}

package() {
    install -Dm755 "target/release/cloudpulse" "$pkgdir/usr/bin/soundcloud"
    ln -sf "/usr/bin/soundcloud" "$pkgdir/usr/bin/cloudpulse"
    install -Dm644 "assets/soundcloud_app_icon.svg" "$pkgdir/usr/share/icons/hicolor/scalable/apps/soundcloud.svg"
    install -Dm644 "assets/soundcloud_app_icon.svg" "$pkgdir/usr/share/icons/hicolor/scalable/apps/cloudpulse.svg"
    install -Dm644 "soundcloud.desktop" "$pkgdir/usr/share/applications/soundcloud.desktop"
    install -Dm644 "soundcloud.desktop" "$pkgdir/usr/share/applications/cloudpulse.desktop"
}
