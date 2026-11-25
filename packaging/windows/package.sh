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
import os, shutil, struct, sys

dist = '$DIST_DIR'
sysroot_bin = os.path.join('$SYSROOT', 'bin')
queue = [os.path.join(dist, 'soundcloud.exe'), os.path.join(dist, 'mpv.exe')]

loaders_dir = os.path.join(dist, 'lib', 'gdk-pixbuf-2.0', '2.10.0', 'loaders')
if os.path.exists(loaders_dir):
    for f in os.listdir(loaders_dir):
        if f.endswith('.dll'):
            queue.append(os.path.join(loaders_dir, f))

SYSTEM_DLLS = {
    'kernel32.dll', 'user32.dll', 'gdi32.dll', 'advapi32.dll', 'shell32.dll',
    'ole32.dll', 'oleaut32.dll', 'comdlg32.dll', 'comctl32.dll', 'ws2_32.dll',
    'wsock32.dll', 'winmm.dll', 'version.dll', 'shlwapi.dll', 'crypt32.dll',
    'bcrypt.dll', 'bcryptprimitives.dll', 'ncrypt.dll', 'secur32.dll', 'userenv.dll',
    'setupapi.dll', 'cfgmgr32.dll', 'imm32.dll', 'uxtheme.dll', 'dwmapi.dll',
    'dxgi.dll', 'd3d11.dll', 'd3d12.dll', 'd3d9.dll', 'opengl32.dll',
    'avrt.dll', 'mmdevapi.dll', 'mfplat.dll', 'mfuuid.dll', 'ksuser.dll',
    'winspool.drv', 'msimg32.dll', 'rpcrt4.dll', 'combase.dll', 'ntdll.dll',
    'msvcrt.dll', 'ucrtbase.dll', 'iphlpapi.dll', 'dnsapi.dll', 'netapi32.dll',
    'wldap32.dll', 'normaliz.dll', 'hid.dll', 'powrprof.dll', 'psapi.dll',
    'dbghelp.dll', 'usp10.dll', 'dwrite.dll', 'd2d1.dll', 'winhttp.dll',
    'wininet.dll', 'shcore.dll', 'wtsapi32.dll', 'oleacc.dll', 'uiautomationcore.dll',
    'propsys.dll', 'strmiids.dll', 'quartz.dll', 'msacm32.dll', 'dsound.dll',
    'mpr.dll', 'authz.dll', 'cryptui.dll', 'imagehlp.dll', 'dxva2.dll', 'evr.dll',
    'dcomp.dll', 'windowscodecs.dll', 'glu32.dll', 'wevtapi.dll', 'pdh.dll',
    'credui.dll', 'cryptnet.dll', 'wintrust.dll', 'vulkan-1.dll', 'opencl.dll',
    'avicap32.dll', 'xmllite.dll', 'gdiplus.dll',
}

def get_imports(fp):
    with open(fp, 'rb') as f:
        data = f.read()
    if len(data) < 0x100 or data[:2] != b'MZ':
        return []
    pe = struct.unpack_from('<I', data, 0x3C)[0]
    if data[pe:pe+4] != b'PE\0\0':
        return []
    coff = pe + 4
    nsec = struct.unpack_from('<H', data, coff + 2)[0]
    opt_sz = struct.unpack_from('<H', data, coff + 16)[0]
    opt = coff + 20
    dd = opt + (112 if struct.unpack_from('<H', data, opt)[0] == 0x20B else 96)
    sec = opt + opt_sz
    secs = [struct.unpack_from('<IIII', data, sec + i * 40 + 8) for i in range(nsec)]
    def r2o(rva):
        for vs, va, rs, rp in secs:
            if va <= rva < va + max(vs, rs):
                return rp + (rva - va)
        return None
    def cstr(off):
        if off is None or off >= len(data): return None
        end = data.find(b'\x00', off)
        return data[off:end].decode('ascii', 'replace') if end != -1 else None
    dlls = []
    imp_rva = struct.unpack_from('<I', data, dd + 8)[0]
    if imp_rva:
        off = r2o(imp_rva)
        while off and off + 20 <= len(data):
            o1, _, _, nr, ft = struct.unpack_from('<IIIII', data, off)
            if o1 == 0 and nr == 0 and ft == 0: break
            n = cstr(r2o(nr))
            if n: dlls.append(n)
            off += 20
    delay_rva = struct.unpack_from('<I', data, dd + 8 * 13)[0]
    if delay_rva:
        off = r2o(delay_rva)
        while off and off + 32 <= len(data):
            _, nr = struct.unpack_from('<II', data, off)
            if nr == 0: break
            n = cstr(r2o(nr))
            if n: dlls.append(n)
            off += 32
    return dlls

bin_files = {f.lower(): f for f in os.listdir(sysroot_bin) if os.path.isfile(os.path.join(sysroot_bin, f))}
copied = set()
missing = {}
while queue:
    curr = queue.pop(0)
    for dll in get_imports(curr):
        dl = dll.lower()
        if dl in copied:
            continue
        if dl in bin_files:
            src = os.path.join(sysroot_bin, bin_files[dl])
            dst = os.path.join(dist, bin_files[dl])
            if not os.path.exists(dst):
                shutil.copy2(src, dst)
            copied.add(dl)
            queue.append(dst)
        elif dl not in SYSTEM_DLLS and not dl.startswith(('api-ms-', 'ext-ms-')):
            missing.setdefault(dll, []).append(os.path.basename(curr))

if missing:
    for m, callers in sorted(missing.items()):
        print(f'ERROR: missing non-system runtime DLL {m} (required by {callers})', file=sys.stderr)
    sys.exit(1)

print(f'Successfully bundled {len(copied)} runtime dependencies into {dist}')
"

echo "=== Distribution packaged successfully ==="
