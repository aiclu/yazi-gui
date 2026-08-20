# Windows Platform Adapters

## Context

The Windows GUI needs native window controls, the notification-area icon, startup registration, remote-drive detection, and WinHTTP. These operations require platform FFI, but placing them in application and domain modules made unsafe code difficult to audit.

## Decision

Windows-specific operations are collected under src/platform/: window.rs, tray.rs, registry.rs, filesystem.rs, and http.rs. Product modules call small safe adapters. Unsafe blocks remain only inside those adapters, close to their handle ownership and pointer invariants; the Yazi process bridge continues to use the safe CommandExt API for hidden process creation.

## Consequences

The application shell no longer imports raw window handles or Windows API symbols, filesystem operations no longer classify remote drives themselves, and update logic no longer contains WinHTTP handle management. Removing Windows behavior is not a goal; the adapter seam makes the required FFI explicit and reviewable.
