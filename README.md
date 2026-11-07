# Radio Log

A desktop logbook for SDR listening and amateur radio.

Record callsigns, frequencies, modes, signal reports, station locations and notes. Add a recording link to an entry, organise entries with your own types, or click a callsign, frequency, mode or location to find matching records.

## Getting started

On Windows, extract the ZIP and open `radio-log.exe`. On Linux, extract the tar.gz and run `radio-log`.

For SDR++ autofill:

1. Open SDR++ and start **Rigctl Server** on `localhost`, port `4532`, with **Radio** selected.
2. Click **Autofill from SDR++** in Radio Log.
3. Check the details and suggested type, then save the entry.

You can also enter everything by hand. Dates and times are UTC. The location field is for the station you are logging.

## Your log

Entries are saved on your computer. **Copy data folder** gives you the folder location. Each change saves a backup of the previous log in its `backups` folder.

Use **Export JSON** to keep a separate backup or move your log to another computer. **Import JSON** lets you merge a file into your log or replace the current log. Merge keeps existing entries when their IDs match. Backups are not removed automatically.

## Downloads and Linux support

Versioned builds will be available under GitHub Releases once the release workflow has run. Windows has been tested locally. The Linux build and desktop checks are still pending.

The Linux download targets x86_64 systems with glibc 2.35 or newer. It needs OpenGL, X11 or Wayland, and a desktop portal with a file-picker backend. ARM and Alpine Linux are not covered by this build.

## Building

The app uses Rust and egui. Build requirements and commands are in [native/README.md](native/README.md). The release process is in [RELEASES.md](RELEASES.md).

The satellite-dish icon uses [Twemoji](https://github.com/twitter/twemoji), licensed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/).
