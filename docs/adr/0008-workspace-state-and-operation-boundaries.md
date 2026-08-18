# Keep workspace state per tab and side effects in narrow modules

## Status

Accepted

## Decision

The main window remains the GPUI shell and coordinator. One `YaziClient` is shared by all GUI tabs; each `Tab` owns its directory snapshot, selection, sorting, search, preview, and asynchronous request tokens. Shared application state is limited to the file clipboard, active transfer/delete work, folder-tree cache, settings, and tray coordination. Filesystem work lives in `fs_ops.rs` with explicit result types, while workspace/domain state lives in `workspace.rs`; input, file-page, settings-page, and reusable UI components live under `ui/`. Asynchronous callbacks validate the target tab and current token before applying results, so switching tabs cannot leave stale work attached to another view. This preserves the existing yazi protocol and user-visible behavior while making ownership and side effects explicit without introducing an event bus, actor framework, compatibility layer, or fallback path.
