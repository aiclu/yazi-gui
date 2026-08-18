# Folder Tree, File Search, Favorites, and Collapsible Preview

## Status

Accepted

## Decision

The file page owns a lazy folder tree backed by direct Rust filesystem reads. Computer View lists only Windows drive roots; expanding a drive or folder scans only its direct child folders. The active real path expands its drive ancestors and is highlighted. Readable folders, hidden folders, and link folders are shown; link folders are never expanded. UNC folders remain reachable through the address path and favorites. Navigation still uses the existing yazi `cd` event, so the yazi protocol is unchanged.

The file page has an always-visible File Search Field beside the page actions. `Ctrl+F` focuses the same field. A non-empty query starts the existing background recursive search for the active directory and matches names or relative paths case-insensitively. Clearing the field immediately restores the normal yazi list; navigation and tab changes clear the search session.

Favorite folders are stored as an explicit `favorites` array in the strict settings schema and are saved immediately. Entries are absolute textual paths, deduplicated case-insensitively, and remain visible when unavailable so the user can remove them explicitly. Computer View itself cannot be favorited. The Favorites Bar preserves insertion order and does not canonicalize or remap paths.

The Preview Panel is hidden at launch and is controlled by an unpersisted toolbar toggle. When shown, it uses a fixed-width right pane and follows the active selection. The folder tree and preview use fixed widths with independent scrolling so the file list remains the primary workspace.
