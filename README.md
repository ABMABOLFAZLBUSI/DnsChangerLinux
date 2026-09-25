# DNS Jump

A compact Linux GUI to switch DNS servers and flush the DNS cache — inspired by **DNS Jumper** for Windows.

Built with **Rust**, **Relm4**, and **libadwaita**. Changes go through **NetworkManager** (`nmcli`); cache flush uses **systemd-resolved** (`resolvectl`).

Works on KDE Plasma (Adwaita chrome) and GNOME.

## Features

- Pick a NetworkManager connection
- Show current effective DNS
- Built-in presets (Cloudflare, Google, Quad9, AdGuard, OpenDNS, Mullvad)
- Custom DNS servers and user-defined groups
- Apply DNS / restore automatic (DHCP) DNS
- Auto-backup before first change; manual backup & restore
- Resolve-latency benchmark (real DNS lookup, not ping) + apply fastest
- One-click DNS cache flush

## Requirements

- NetworkManager
- GTK 4 + libadwaita
- `resolvectl` (systemd) recommended for cache flush

Arch / CachyOS:

```bash
sudo pacman -S --needed gtk4 libadwaita networkmanager rust base-devel
```

## Build & run (development)

```bash
cargo run --release
```

## Install on Arch (from this repo)

```bash
./install.sh
```

Or manually:

```bash
cargo build --release
sudo install -Dm755 "${CARGO_TARGET_DIR:-target}/release/dns-jump" /usr/bin/dns-jump
sudo install -Dm644 data/dns-jump.desktop /usr/share/applications/dns-jump.desktop
sudo install -Dm644 data/icons/hicolor/scalable/apps/io.github.DnsJump.svg \
  /usr/share/icons/hicolor/scalable/apps/io.github.DnsJump.svg
sudo gtk-update-icon-cache -f /usr/share/icons/hicolor
sudo update-desktop-database
```

For packaging, see [`packaging/PKGBUILD.local`](packaging/PKGBUILD.local) (build from this tree) and [`packaging/PKGBUILD`](packaging/PKGBUILD) (AUR release tarball).

## Usage

1. Select your Wi‑Fi or Ethernet connection.
2. Choose a DNS preset (or add a custom one from the menu).
3. Click **Apply DNS** (Plasma may ask for your password via polkit).
4. Optionally **Flush Cache**, **Benchmark**, or **Apply Fastest**.

Config and backups live in `~/.config/dns-jump/`.

## How it works

DNS Jump does **not** edit `/etc/resolv.conf` by hand. On modern Arch desktops that file points at systemd-resolved. Instead it sets:

```text
ipv4.dns / ipv6.dns
ipv4.ignore-auto-dns / ipv6.ignore-auto-dns
```

on the selected NetworkManager connection, then brings the connection up and flushes caches with `resolvectl flush-caches`.

## License

MIT
