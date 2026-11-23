#!/usr/bin/env bash
set -euo pipefail

# CloudPulse Windows Local Packaging Script (Run inside MSYS2 UCRT64 shell)
echo "=== Building CloudPulse for Windows (x86_64) ==="
cargo build --release

DIST_DIR="soundcloud-windows-x86_64"
mkdir -p "$DIST_DIR"

echo "=== Copying binary and assets ==="
cp target/release/cloudpulse.exe "$DIST_DIR/soundcloud.exe"
if [ -f assets/soundcloud.ico ]; then
  cp assets/soundcloud.ico "$DIST_DIR/"
fi

if [ -f /ucrt64/bin/mpv.exe ]; then
  cp /ucrt64/bin/mpv.exe "$DIST_DIR/"
fi

mkdir -p "$DIST_DIR/share/glib-2.0/schemas"
cp -r /ucrt64/share/glib-2.0/schemas/* "$DIST_DIR/share/glib-2.0/schemas/" 2>/dev/null || true
glib-compile-schemas "$DIST_DIR/share/glib-2.0/schemas/"

mkdir -p "$DIST_DIR/share/icons"
cp -r /ucrt64/share/icons/Adwaita "$DIST_DIR/share/icons/" 2>/dev/null || true
cp -r /ucrt64/share/icons/hicolor "$DIST_DIR/share/icons/" 2>/dev/null || true

cat << 'EOF' > "$DIST_DIR/run.bat"
@echo off
setlocal
cd /d "%~dp0"
start "" "%~dp0soundcloud.exe"
EOF

echo "=== Collecting dynamic runtime DLL dependencies ==="
python3 -c "
import os, shutil, subprocess

dist = '$DIST_DIR'
bins = [os.path.join(dist, 'soundcloud.exe')]
mpv_bin = os.path.join(dist, 'mpv.exe')
if os.path.exists(mpv_bin):
    bins.append(mpv_bin)

copied = set()
queue = list(bins)

while queue:
    curr = queue.pop(0)
    res = subprocess.run(['ldd', curr], capture_output=True, text=True)
    for line in res.stdout.splitlines():
        parts = line.strip().split('=>')
        if len(parts) == 2:
            dll_path = parts[1].strip().split(' ')[0]
            if dll_path.startswith('/ucrt64') and os.path.exists(dll_path):
                dll_name = os.path.basename(dll_path)
                dest = os.path.join(dist, dll_name)
                if dll_name not in copied:
                    copied.add(dll_name)
                    shutil.copy2(dll_path, dest)
                    queue.append(dest)
print(f'Successfully bundled {len(copied)} runtime dependencies into {dist}')
"

zip -r soundcloud-v0.1.1-windows-x86_64.zip "$DIST_DIR"
echo "=== Distribution packaged to soundcloud-v0.1.1-windows-x86_64.zip ==="
