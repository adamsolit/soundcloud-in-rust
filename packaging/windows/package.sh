#!/usr/bin/env bash
set -euo pipefail

echo "=== Building SoundCloud for Windows (x86_64) ==="
cargo build --release --target x86_64-pc-windows-gnu

DIST_DIR="soundcloud-windows-x86_64"
rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"

echo "=== Copying binary and assets ==="
cp target/x86_64-pc-windows-gnu/release/cloudpulse.exe "$DIST_DIR/soundcloud.exe"
if [ -f assets/soundcloud.ico ]; then
  cp assets/soundcloud.ico "$DIST_DIR/"
fi

SYSROOT="${MSYSTEM_PREFIX:-/ucrt64}"
if [ -f "$SYSROOT/bin/mpv.exe" ]; then
  cp "$SYSROOT/bin/mpv.exe" "$DIST_DIR/"
fi

mkdir -p "$DIST_DIR/share/glib-2.0/schemas"
cp -r "$SYSROOT/share/glib-2.0/schemas/"* "$DIST_DIR/share/glib-2.0/schemas/" 2>/dev/null || true
glib-compile-schemas "$DIST_DIR/share/glib-2.0/schemas/"

mkdir -p "$DIST_DIR/share/icons"
cp -r "$SYSROOT/share/icons/Adwaita" "$DIST_DIR/share/icons/" 2>/dev/null || true
cp -r "$SYSROOT/share/icons/hicolor" "$DIST_DIR/share/icons/" 2>/dev/null || true

mkdir -p "$DIST_DIR/share/fonts"
if [ -d /tmp/bundled-fonts ]; then
  cp /tmp/bundled-fonts/* "$DIST_DIR/share/fonts/" 2>/dev/null || true
fi

if [ -d "$SYSROOT/etc/fonts" ]; then
  mkdir -p "$DIST_DIR/etc"
  cp -r "$SYSROOT/etc/fonts" "$DIST_DIR/etc/"
fi

mkdir -p "$DIST_DIR/lib/gdk-pixbuf-2.0/2.10.0/loaders"
if [ -f "$SYSROOT/lib/gdk-pixbuf-2.0/2.10.0/loaders.cache" ]; then
  cp "$SYSROOT/lib/gdk-pixbuf-2.0/2.10.0/loaders.cache" "$DIST_DIR/lib/gdk-pixbuf-2.0/2.10.0/"
fi
cp -r "$SYSROOT/lib/gdk-pixbuf-2.0/2.10.0/loaders/"*.dll "$DIST_DIR/lib/gdk-pixbuf-2.0/2.10.0/loaders/" 2>/dev/null || true

cat << 'EOF' > "$DIST_DIR/run.bat"
@echo off
setlocal
cd /d "%~dp0"
start "" "%~dp0soundcloud.exe"
EOF

echo "=== Resolving and copying runtime DLLs ==="
python3 -c "
import os, shutil, subprocess

dist = '$DIST_DIR'
sysroot_bin = os.path.join('$SYSROOT', 'bin')
queue = [os.path.join(dist, 'soundcloud.exe'), os.path.join(dist, 'mpv.exe')]

loaders_dir = os.path.join(dist, 'lib', 'gdk-pixbuf-2.0', '2.10.0', 'loaders')
if os.path.exists(loaders_dir):
    for f in os.listdir(loaders_dir):
        if f.endswith('.dll'):
            queue.append(os.path.join(loaders_dir, f))

def get_imports(fp):
    res = subprocess.run(['objdump', '-p', fp], capture_output=True, text=True)
    dlls = []
    for line in res.stdout.splitlines():
        if 'DLL Name:' in line:
            dlls.append(line.split('DLL Name:')[1].strip())
    return dlls

copied = set()
while queue:
    curr = queue.pop(0)
    for dll in get_imports(curr):
        dl = dll.lower()
        if dl in copied:
            continue
        cand = os.path.join(sysroot_bin, dll)
        if not os.path.exists(cand):
            for f in os.listdir(sysroot_bin):
                if f.lower() == dl:
                    cand = os.path.join(sysroot_bin, f)
                    break
        if os.path.exists(cand) and os.path.isfile(cand):
            dst = os.path.join(dist, os.path.basename(cand))
            if not os.path.exists(dst):
                shutil.copy2(cand, dst)
                copied.add(dl)
                queue.append(dst)
print(f'Successfully bundled {len(copied)} runtime dependencies into {dist}')
"

echo "=== Distribution packaged successfully ==="
