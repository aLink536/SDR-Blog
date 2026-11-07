# Radio Log

A desktop logbook for SDR listening and amateur radio.

## Run

Open `radio-log.exe` on Windows, or `radio-log` on Linux. For autofill, start SDR++ Rigctl Server on `localhost:4532` with Radio selected, then click **Autofill from SDR++**. Check the suggested type before saving. Manual entries work without SDR++.

Dates and times are UTC. Station location means the station you are logging.

## Storage

The app saves `radio-log.json` in your user application-data folder. **Copy data folder** copies its location. Each change backs up the previous file before saving. Backups are kept until you remove them.

**Export JSON** saves a copy wherever you choose. **Import JSON** accepts version 1 and version 2 logs, with Merge and Replace options. Merge keeps existing entries with matching IDs. If a saved log cannot be read, the app leaves it untouched and disables editing.

## Linux

The release archive targets x86_64 desktops with glibc 2.35 or newer, OpenGL, X11 or Wayland, and an XDG Desktop Portal file picker. Install the portal backend for your desktop, such as GTK, GNOME or KDE. ARM, older glibc and Alpine/musl are not covered.

For an application-menu entry, put `radio-log` on your PATH, copy `radio-log.desktop` into `~/.local/share/applications/`, and copy `radio-log.png` into `~/.local/share/icons/hicolor/72x72/apps/`. Create those folders if needed.

Windows has been tested. Linux compilation and desktop testing are pending the first GitHub build.

## Build

Use Rust 1.99.1 or a compatible newer stable version. From this directory:

```
cargo test --locked
cargo build --release --locked
```

Windows needs the MSVC Rust toolchain and Microsoft C++ build tools, or GNU Rust with MinGW including its resource compiler.

On Debian/Ubuntu, install `build-essential pkg-config libx11-dev libxkbcommon-dev libwayland-dev libegl1-mesa-dev libgl1-mesa-dev`. Other distributions need equivalent development packages.

The executable is written to `target/release/`. `--data-dir PATH` uses a separate log folder for testing. `--probe` reads SDR++ once and exits.

GitHub builds both platforms on pushes and pull requests. A tag matching the package version, such as `v0.1.0`, publishes the archives and checksums after both builds pass. See RELEASES.md in the repository for details.

Icon credits and license are included in THIRD-PARTY-NOTICES.txt and assets/LICENSE-TWEMOJI.txt.
