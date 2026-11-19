# SoundCloud Native Linux Desktop Client

A lightweight, high-performance native Linux desktop client for SoundCloud, built with **Rust**, **GTK4**, **Libadwaita**, and **MPV/PipeWire**. Designed specifically for modern Wayland environments (Hyprland, Sway, GNOME, KDE) and Arch Linux / CachyOS, with zero Electron/Chromium bloat, sub-second startup, MPRIS media controls, and minimal memory footprint.

---

## Architecture Overview

```
                               ┌─────────────────────────────────────────┐
                               │   Hyprland / Noctalia Shell / Wayland   │
                               └──────┬───────────────────────────┬──────┘
                                      │ MPRIS D-Bus               │ Wayland Surface
                                      ▼                           ▼
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                                   SoundCloud (Rust)                                    │
│                                                                                        │
│  ┌──────────────────────┐   ┌───────────────────────────┐   ┌───────────────────────┐  │
│  │   UI Presentation    │   │       Audio Engine        │   │    Network & API      │  │
│  │  GTK4 + Libadwaita   │◄──┤  Headless MPV IPC Daemon  │◄──┤  Reqwest + Tokio      │  │
│  │  Cairo 60fps Waveform│   │   PipeWire / ALSA Output  │   │  SoundCloud API v2    │  │
│  │  Dark Theme (#111111)│   │  Bounded Buffer (24M/6M)  │   │  Auto-Healing Keys    │  │
│  └──────────┬───────────┘   └─────────────┬─────────────┘   └───────────┬───────────┘  │
│             │                             │                             │              │
│             └──────────────────────┬──────┴─────────────────────────────┘              │
│                                    ▼                                                   │
│                     ┌─────────────────────────────┐                                    │
│                     │     Local Storage Cache     │                                    │
│                     │  SQLite (rusqlite WAL mode) │                                    │
│                     │  Bounded LRU Texture Cache  │                                    │
│                     │  Keyring-rs Secret Service  │                                    │
│                     └─────────────────────────────┘                                    │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

1. **UI Layer (`gtk4` + `libadwaita` + Cairo)**:
   - Native Wayland client with smooth fractional scaling, hardware rendering, and unified modern Adwaita styling.
   - Clean monochrome dark theme (`#111111` background, `#161616` cards, `#FFFFFF` pure white accents).
   - **Interactive Waveform Widget**: Custom 60fps `gtk::DrawingArea` with Cairo rendering. Displays played segments, unplayed progress, hover scrubbing preview, and comment markers.
   - Profile management, playlist management, creator profile views with follow toggles, track catalog, and full upload capabilities.

2. **Audio Engine (`mpv` IPC + PipeWire)**:
   - Headless background MPV daemon managed over high-speed local UNIX IPC domain socket.
   - Direct hardware-accelerated playback via PipeWire/PulseAudio/ALSA.
   - Strict demuxer memory limits (`--demuxer-max-bytes=24M`, `--demuxer-max-back-bytes=6M`) to keep resident memory bounded and flat.
   - Full playback queue with FIFO, Shuffle, and Repeat modes (Off, All, One).

3. **SoundCloud API v2 & Auto-Healing Session**:
   - Live endpoints for Search, Charts, Activity Streams, Comments, Likes, Playlists, Artist Profiles, and Uploads.
   - **Dynamic Client-ID Discovery**: If a public Client ID is revoked or expires, automatically parses web bundles to extract the active client key and refreshes seamlessly.
   - Support for personal OAuth tokens and persistent session storage.

4. **Resource Optimization**:
   - Bounded in-memory texture cache (max 40 active covers) with automatic FIFO eviction and persistent disk caching in `~/.cache/soundcloud/artwork`.
   - Tuned Tokio runtime (2 worker threads, constrained thread pools) to prevent thread stack virtual memory overhead.

5. **Desktop & Media Control (`mpris-server` D-Bus)**:
   - Full MPRIS2 D-Bus integration for lockscreens, hardware media keys, `playerctl`, and status bar widgets (Waybar, Noctalia, etc.).

---

## Installation & Packaging

### Prerequisites (Arch Linux / CachyOS / Manjaro)
```bash
sudo pacman -S --needed rust cargo gtk4 libadwaita mpv openssl sqlite
```

### Option A: Install via Arch Linux PKGBUILD (Recommended for Arch)
```bash
# Clone the repository
git clone https://github.com/adamsolit/soundcloud-in-rust.git
cd soundcloud-in-rust

# Build and install package via makepkg
makepkg -si
```

### Option B: Build and Install with Make
```bash
make
sudo make install
```

### Option C: Manual Cargo Build
```bash
cargo build --release

# Copy binary to your local user PATH
install -Dm755 target/release/cloudpulse ~/.local/bin/soundcloud
ln -sf ~/.local/bin/soundcloud ~/.local/bin/cloudpulse

# Install desktop entry and icon
install -Dm644 soundcloud.desktop ~/.local/share/applications/soundcloud.desktop
install -Dm644 assets/soundcloud_app_icon.svg ~/.local/share/icons/hicolor/scalable/apps/soundcloud.svg
update-desktop-database ~/.local/share/applications/
```

---

## Running the Application

You can launch SoundCloud directly from your desktop launcher (Rofi, Wofi, Noctalia, App Launcher) or from terminal:
```bash
soundcloud
# or
gtk4-launch soundcloud
```

---

## Test Suite
Run the automated test suite with:
```bash
cargo test
```

---

## License
MIT
