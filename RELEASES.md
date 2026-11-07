# Publishing Radio Log releases

Normal pushes and pull requests build and test Windows x86_64 and Linux x86_64. You can also run **Actions > Native builds and releases > Run workflow** for downloadable test artifacts without publishing a release.

## Publish a version

1. Update `version` in `native/Cargo.toml`, then run `cargo check` in `native/` to update `Cargo.lock`.
2. Commit and push the reviewed source and workflow. This requires the owner's approval; do not commit automatically.
3. Create and push a matching tag, for example `v0.1.0` for package version `0.1.0`.
4. GitHub Actions tests and builds both platforms. The Linux job also checks that the window stays running under a virtual X11 display with software rendering.
5. Only when both jobs succeed, the workflow creates a GitHub Release with Windows ZIP, Linux tar.gz, generated release notes and `SHA256SUMS.txt`. Assets are uploaded while the release is a draft, then it is published. Tags containing a version suffix such as `v0.2.0-beta.1` create prereleases.

The tag must match the package version. A failed upload can be rerun to repair the draft. The workflow refuses to overwrite an already published release; use a new version instead. The automatic repository token is sufficient, with write access limited to the release job. No personal access token is required unless repository/organization policy disables these permissions.

## Linux support boundary

The Linux archive is for x86_64 desktop distributions with glibc 2.35 or newer (the Ubuntu 22.04 build baseline). It needs OpenGL, X11 or Wayland, and an XDG desktop portal plus an appropriate file-picker backend for import/export. It is not a universal package for all distributions: older glibc, Alpine/musl and ARM are not covered. The archive preserves executable permissions; extract and launch `radio-log`.

The automated startup check is not a full desktop test. Wayland, file-picker integration, graphics drivers and live SDR++ should still be verified on real Linux desktops before claiming broad support.

## Current verification

Windows has been built and tested locally. The release workflow has been checked locally but has not yet run on GitHub. Linux compilation, startup and published release downloads remain pending the first authorized push/tag.
