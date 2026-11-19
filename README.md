# SoundCloud for Linux

A native GTK4 / Libadwaita desktop client for SoundCloud, written in Rust.

Modern web browsers and Electron-based wrappers are heavy, sluggish, and lack proper desktop integration. This client is built from scratch as a fast, native Linux application designed for Wayland and X11, featuring direct PipeWire/ALSA playback, interactive Cairo waveform scrubbing, and standard MPRIS media integration.

---

## Features

- **Native Linux UI**: Built with GTK4 and Libadwaita. Seamless fractional scaling, smooth scrolling, and a clean monochrome dark theme.
- **Interactive Waveform**: Custom Cairo-rendered waveform widget with click-to-seek, hover time inspection, and real-time scrubbing at 60 FPS.
- **Low Resource Usage**: Bounded in-memory texture caching (capped at 40 covers) and demuxer memory caps to maintain a flat RAM footprint (~70–110 MB RSS).
- **MPV Audio Backend**: Headless MPV background daemon communicating over local UNIX IPC for gapless, hardware-accelerated streaming via PipeWire/PulseAudio.
- **Desktop Integration**:
  - Full MPRIS2 D-Bus support (hardware media keys, lock screen playback, `playerctl`, Waybar / Noctalia widgets).
  - Automatically registers its `.desktop` launcher and vector icon on first launch.
- **Account & Library Management**:
  - Creator and artist profiles with track catalogs and follow toggles.
  - Personal library sync (liked tracks, custom playlists).
  - Track uploads with metadata and artwork support.
  - Persistent login via Linux Secret Service keyring (with guest mode fallback).

---

## Installation

### Arch Linux / CachyOS / Manjaro

Install system dependencies:
```bash
sudo pacman -S --needed rust cargo gtk4 libadwaita mpv openssl sqlite
```

Build and install with `makepkg`:
```bash
git clone https://github.com/adamsolit/soundcloud-in-rust.git
cd soundcloud-in-rust
makepkg -si
```

### Manual Build (Any Distribution)

```bash
git clone https://github.com/adamsolit/soundcloud-in-rust.git
cd soundcloud-in-rust
cargo build --release

# Install binary to user PATH
install -Dm755 target/release/cloudpulse ~/.local/bin/soundcloud

# Install desktop launcher and icon
install -Dm644 soundcloud.desktop ~/.local/share/applications/soundcloud.desktop
install -Dm644 assets/soundcloud_app_icon.svg ~/.local/share/icons/hicolor/scalable/apps/soundcloud.svg
update-desktop-database ~/.local/share/applications/
```

---

## Usage

Launch SoundCloud from your desktop application launcher (Rofi, Wofi, Noctalia, App Grid) or run:
```bash
soundcloud
```

---

## Technical Stack

- **GUI**: GTK4 (`gtk4-rs`), Libadwaita (`libadwaita-rs`), Cairo rendering
- **Audio Engine**: Headless `mpv` process over UNIX domain socket IPC
- **Async Runtime**: Tokio (tuned 2-worker pool) + Reqwest (HTTP/2 connection pooling)
- **Database & Cache**: SQLite (WAL mode) for offline cache; Secret Service for credentials
- **Media Controls**: `mpris-server` over D-Bus (`org.mpris.MediaPlayer2.cloudpulse`)

---

## License

MIT
