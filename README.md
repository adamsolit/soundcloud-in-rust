# SoundCloud Desktop Client

A native desktop client for SoundCloud, written in Rust with GTK4 and Libadwaita. Available for Linux and Windows.

---

## Downloads (v0.1.0)

| Platform | Download | Instructions |
| :--- | :--- | :--- |
| **Windows** | [`soundcloud-v0.1.0-windows-x86_64.zip`](https://github.com/adamsolit/soundcloud-in-rust/releases/download/v0.1.0/soundcloud-v0.1.0-windows-x86_64.zip) | Extract and run `run.bat` (or `soundcloud.exe`) |
| **Linux** | [`soundcloud-v0.1.0-linux-x86_64.tar.gz`](https://github.com/adamsolit/soundcloud-in-rust/releases/download/v0.1.0/soundcloud-v0.1.0-linux-x86_64.tar.gz) | Extract and run `./install.sh` |
| **Windows Exe Only** | [`soundcloud.exe`](https://github.com/adamsolit/soundcloud-in-rust/releases/download/v0.1.0/soundcloud.exe) | Standalone executable |

---

## Features

- **Fast and lightweight**: Native app with low memory usage (~70-100 MB RAM), built with GTK4 and powered by an MPV audio backend.
- **Interactive waveform**: Visual waveform for tracks with hover timestamp preview and click-to-seek.
- **Named bookmarks**: Press `b` while listening to save a named bookmark at the current timestamp, then jump back to it later from the waveform.
- **Offline downloads**: Download tracks to an "Offline Stash" for playback without an internet connection.
- **Playback controls**: Speed adjustment (0.5x to 2.0x), sleep timer, and track link sharing tucked into a clean player menu.
- **Browser session auto-detection**: Automatically imports your logged-in SoundCloud session from Firefox, Zen Browser, Chrome, Brave, or Edge.
- **Desktop integration**: Media keys, lock screen controls, and system volume support (MPRIS on Linux, native media controls on Windows).
- **Clean interface**: Dark theme, collapsible sidebar (`Ctrl+B`), and full support for both English and Arabic text without truncation.

---

## Installation

### Windows
1. Download `soundcloud-v0.1.0-windows-x86_64.zip`.
2. Extract the folder anywhere.
3. Run `run.bat` (or `soundcloud.exe`). Everything needed is included in the zip.

### Linux (Pre-built)
```bash
tar -xzf soundcloud-v0.1.0-linux-x86_64.tar.gz
cd soundcloud-v0.1.0-linux-x86_64
./install.sh
```
*(Requires `libadwaita` and `mpv` installed on your system)*

### Linux (Arch Linux / CachyOS)
```bash
git clone https://github.com/adamsolit/soundcloud-in-rust.git
cd soundcloud-in-rust
makepkg -si
```

### Build from Source
```bash
git clone https://github.com/adamsolit/soundcloud-in-rust.git
cd soundcloud-in-rust
cargo build --release
```

---

## Shortcuts

| Key | Action |
| :--- | :--- |
| `Space` | Play / Pause |
| `Left` / `Right` | Seek 5 seconds backward / forward |
| `Up` / `Down` | Volume up / down |
| `m` | Mute / Unmute |
| `b` | Add named bookmark |
| `Ctrl + B` | Toggle sidebar |
| `Ctrl + Q` | Quit |

---

## Contributors

- **adamsolit** ([@adamsolit](https://github.com/adamsolit))
- **abodyfawaz** ([@abodyfawaz](https://github.com/abodyfawaz))
- **Antigravity** ([Google DeepMind](https://deepmind.google)) — AI Pair Programmer

---

## License

MIT
