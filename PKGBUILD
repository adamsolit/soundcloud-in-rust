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
    'openssl'
    'sqlite'
)
optdepends=(
    'gstreamer: fallback backend when MPV is unavailable'
    'gst-plugins-base: basic GStreamer codecs'
    'gst-plugins-good: expanded GStreamer playback support'
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
    ln -sf "soundcloud" "$pkgdir/usr/bin/cloudpulse"
    install -Dm644 "assets/soundcloud_app_icon.svg" "$pkgdir/usr/share/icons/hicolor/scalable/apps/org.cloudpulse.SoundCloud.svg"
    install -Dm644 "assets/soundcloud_app_icon.svg" "$pkgdir/usr/share/icons/hicolor/scalable/apps/soundcloud.svg"
    install -Dm644 "assets/soundcloud_app_icon.svg" "$pkgdir/usr/share/icons/hicolor/scalable/apps/cloudpulse.svg"
    install -Dm644 "org.cloudpulse.SoundCloud.desktop" "$pkgdir/usr/share/applications/org.cloudpulse.SoundCloud.desktop"
    ln -sf "org.cloudpulse.SoundCloud.desktop" "$pkgdir/usr/share/applications/soundcloud.desktop"
    ln -sf "org.cloudpulse.SoundCloud.desktop" "$pkgdir/usr/share/applications/cloudpulse.desktop"
}
