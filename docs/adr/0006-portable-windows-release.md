# Portable Windows Release Packages

## Status

Accepted

## Decision

Release builds use the executable's directory as the runtime root. The `assets/yazi` directory is copied beside the GUI executable, so a packaged build never depends on the source checkout path. Debug builds continue to use the repository's development assets.

The default build uses `yazi` and `ya` from `PATH`. The `bundled-yazi` Cargo feature produces a separate build that uses only `yazi.exe` and `ya.exe` beside the executable. There is no runtime fallback between these modes.

Every `v*` tag publishes two Windows x86_64 zip files. The bundled archive includes the pinned yazi 26.8.15 binaries, their MIT license, the GUI executable, and the GUI yazi configuration. The workflow verifies the upstream archive SHA-256 before packaging and publishes checksums for the final archives.

The application starts in the process current working directory. Users can navigate to another location through Computer View or the address bar; no developer machine path is embedded in the release behavior.
