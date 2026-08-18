# Use manual GitHub releases for verified updates

The Settings page provides a manual update check against the latest stable release of `aiclu/yazi-gui`. The application selects the package that matches the running build: GUI-only builds use the package that resolves `yazi` and `ya` from `PATH`, while bundled builds use the package containing the fixed bundled yazi version. Drafts and prereleases are not update candidates.

Update requests use the Windows HTTP stack with automatic proxy selection so the current Windows proxy configuration, including automatic configuration, remains authoritative. The application does not add an application-specific proxy setting or perform a network request during startup.

The selected archive is downloaded to a temporary partial file with byte progress and cancellation. The published SHA-256 manifest is fetched first; a checksum mismatch deletes the partial archive and leaves the current installation untouched. A completed download remains available until the user explicitly chooses restart.

Restart runs a temporary updater process after the GUI exits. The updater extracts the verified archive, validates the executable and runtime yazi configuration, swaps the installation directory as one transaction, and launches the new executable. It keeps the existing settings directory outside the package. Failed activation restores the previous installation; no silent update, alternate source, or runtime fallback is used.
