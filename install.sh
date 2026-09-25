#!/usr/bin/env bash
# Install DNS Jump system-wide (requires sudo once).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"

echo "==> Building release binary…"
cargo build --release

BIN="target/release/dns-jump"
if [[ ! -x "$BIN" && -n "${CARGO_TARGET_DIR:-}" ]]; then
  BIN="$CARGO_TARGET_DIR/release/dns-jump"
fi
if [[ ! -x "$BIN" ]]; then
  echo "Could not find dns-jump binary. Build failed?" >&2
  exit 1
fi

echo "==> Installing to /usr …"
sudo install -Dm755 "$BIN" /usr/bin/dns-jump
sudo install -Dm644 data/dns-jump.desktop /usr/share/applications/dns-jump.desktop
sudo install -Dm644 data/icons/hicolor/scalable/apps/io.github.DnsJump.svg \
  /usr/share/icons/hicolor/scalable/apps/io.github.DnsJump.svg
sudo install -Dm644 data/io.github.DnsJump.metainfo.xml \
  /usr/share/metainfo/io.github.DnsJump.metainfo.xml
sudo gtk-update-icon-cache -f /usr/share/icons/hicolor 2>/dev/null || true
sudo update-desktop-database 2>/dev/null || true

echo "Done. Launch with: dns-jump"
