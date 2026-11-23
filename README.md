# SoundCloud Desktop Client

A high-performance, native GTK4 / Libadwaita desktop client for SoundCloud written in Rust.

Modern web browsers and Electron-based wrappers are heavy, sluggish, and lack proper desktop integration. This client is engineered from scratch as a fast, native multi-platform application designed for Linux (Wayland & X11) and Windows (10 & 11), featuring direct hardware audio playback via MPV, interactive Cairo waveform scrubbing, offline local caching, and deep operating system integration.

---

## Downloads (Release v0.1.0)

| Platform | Package | Format | Details |
| :--- | :--- | :--- | :--- |
| **Linux (x86_64)** | [`soundcloud-v0.1.0-linux-x86_64.tar.gz`](https://github.com/adamsolit/soundcloud-in-rust/releases/download/v0.1.0/soundcloud-v0.1.0-linux-x86_64.tar.gz) | `.tar.gz` | Standalone Linux binary, desktop launcher, icons, and automated installer |
| **Windows (x86_64)** | [`soundcloud-v0.1.0-windows-x86_64.zip`](https://github.com/adamsolit/soundcloud-in-rust/releases/download/v0.1.0/soundcloud-v0.1.0-windows-x86_64.zip) | `.zip` | Portable package with `soundcloud.exe`, `mpv.exe`, GTK4 runtime DLLs, and `run.bat` |
| **Windows Executable** | [`soundcloud.exe`](https://github.com/adamsolit/soundcloud-in-rust/releases/download/v0.1.0/soundcloud.exe) | `.exe` | Standalone 64-bit Windows PE binary |

---

## Core Features

### Swiss Minimalist Monochrome Aesthetic
- **Restrained Visual Polish**: Obsidian backgrounds (`#0d0d0f`, `#161618`), subtle border dividers (`#262628`), and pure white accents (`#ffffff`).
- **Precision Typography**: Clean, high-legibility sans-serif typography (Inter, Adwaita Sans, and Noto Sans Arabic) with zero label truncation.
- **Micro-Interaction Physics**: Cubic-bezier easing curve (`cubic-bezier(0.16, 1, 0.3, 1)`) with tactile card elevation and responsive button feedback.
- **Collapsible Sidebar**: Instant icon-only sidebar collapse (`Ctrl+B` / `F9`) for focused listening or compact window arrangements.

### Audio Engine & Performance
- **Headless MPV Integration**: Communicates via local IPC sockets (`$XDG_RUNTIME_DIR` on Linux, Win32 Named Pipes on Windows) for low-latency, gapless, hardware-accelerated streaming.
- **Native Audio Backends**: Direct PipeWire/ALSA output on Linux; native WASAPI output on Windows.
- **Minimal Memory Overhead**: Flat memory footprint (~70–110 MB RSS) maintained through bounded texture caching (capped at 40 covers) and demuxer buffering limits.

### Interactive Waveform & Scrubbing
- **Custom Cairo Engine**: 60 FPS real-time scrubbing, hover time-inspection, and cached sample rendering.
- **Named Waveform Bookmarks**: Press `b` to prompt a bookmark creation dialog. Save labeled timestamps across any track and hover over the waveform to inspect and jump directly to points of interest.

### Productivity, Offline & Library
- **Offline Stash**: Download tracks directly for offline playback via the track context menu or player bar. Stashed tracks are indexed locally in SQLite with instant offline seeking.
- **Streamlined Player Overflow Hub**: A non-intrusive `...` overlay menu housing Sleep Timer (15m, 30m, 45m, 60m, or track end), Playback Speed (0.5x to 2.0x pitch-preserved), Share link copying, and Offline Download toggles.
- **Session Auto-Detection**: Background zero-lag cookie scanning for active SoundCloud sessions across Firefox, Zen Browser, Google Chrome, Brave, and Microsoft Edge.
- **Catalog & Account Sync**: Access personal streams, discover algorithmic feeds, manage liked tracks and playlists, search the full catalog, and explore artist profiles.

### Desktop Integration
- **Linux**: Full MPRIS2 D-Bus integration (hardware media keys, lock screen media controls, `playerctl`, Waybar, and Noctalia widget support).
- **Windows**: Native taskbar controls, volume management, and portable zero-registry deployment.

---

## Installation & Running

### Windows (10 / 11 64-bit)

1. Download **`soundcloud-v0.1.0-windows-x86_64.zip`** from the [Releases page](https://github.com/adamsolit/soundcloud-in-rust/releases/tag/v0.1.0).
2. Extract the archive into any folder (e.g. `C:\Program Files\SoundCloud` or your Desktop).
3. Double-click **`run.bat`** (or `soundcloud.exe`) to launch.
4. *No installation or external dependencies required.* All GTK4 runtime libraries, GLib schemas, icons, and MPV engines are completely self-contained.

### Linux Pre-built Bundle

```bash
tar -xzf soundcloud-v0.1.0-linux-x86_64.tar.gz
cd soundcloud-v0.1.0-linux-x86_64
./install.sh
```
*(Installs to `~/.local` by default without needing root permissions, or pass `--system` for `/usr/local`)*.

### Arch Linux / CachyOS / Manjaro (Source Package)

```bash
# Install build dependencies
sudo pacman -S --needed base-devel git rust cargo gtk4 libadwaita mpv openssl sqlite

# Clone and build package
git clone https://github.com/adamsolit/soundcloud-in-rust.git
cd soundcloud-in-rust
makepkg -si
```

### Manual Compilation from Source

Ensure `rustc` (1.80+), `gtk4`, `libadwaita-1`, and `mpv` development libraries are installed.

```bash
git clone https://github.com/adamsolit/soundcloud-in-rust.git
cd soundcloud-in-rust

cargo build --release

# The compiled binary will be located at:
# target/release/cloudpulse
```

---

## Keyboard Shortcuts

| Shortcut | Action |
| :--- | :--- |
| `Space` | Play / Pause playback |
| `Left Arrow` | Seek backward 5 seconds |
| `Right Arrow` | Seek forward 5 seconds |
| `Up Arrow` | Increase volume |
| `Down Arrow` | Decrease volume |
| `m` | Toggle mute |
| `b` | Create named bookmark at current timestamp |
| `Ctrl + B` / `F9` | Toggle sidebar collapse |
| `Ctrl + Q` | Quit application |

---

## Technical Architecture

```
soundcloud-in-rust/
├── src/
│   ├── main.rs                 # Application entry point, Tokio runtime initialization
│   ├── app.rs                  # GTK4 / Libadwaita window coordinator & action handlers
│   ├── api/                    # SoundCloud API client, token handling, cookie scanners
│   ├── audio/                  # MPV process supervisor, IPC communication, playback events
│   ├── db/                     # SQLite persistence (history, cached profiles, offline tracks, bookmarks)
│   ├── models/                 # Track, playlist, user, and comment data models
│   ├── mpris/                  # Linux MPRIS2 D-Bus interface and Windows stubs
│   ├── paths.rs                # Cross-platform XDG / Win32 LocalAppData path resolver
│   └── ui/                     # GTK4 widgets, views, Cairo waveform rendering, theme styles
├── packaging/
│   └── windows/                # Windows packaging scripts and CI definitions
├── assets/                     # Application icons (SVG / ICO)
└── Cargo.toml                  # Cargo dependencies and workspace configuration
```

---

## License

This project is licensed under the [MIT License](LICENSE).
