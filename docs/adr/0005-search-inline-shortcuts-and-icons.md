# Search, inline editing, shortcut recording, and application icon

## Status

Accepted

## Decisions

The GUI provides a transient recursive search for the active real directory. `Ctrl+F` enters a search bar in place of the address path; matching is case-insensitive against both the entry name and its relative path. The scan runs on the background executor, includes hidden entries, does not recurse through symlink directories, and uses generations so stale results cannot replace a newer query. Search is deliberately not a yazi protocol command.

Rename, new-file, and new-folder operations use an editor in the filename column. Rename selects only the main name stem on entry, while the extension remains visible and editable. New items use one temporary row outside the active sort order. Enter or an accepted outside action commits; Escape cancels. Invalid names and filesystem conflicts keep the editor active and report the error.

Shortcut settings use click-to-record for one key chord. Modifier-only presses are ignored, Escape cancels, and Backspace clears. A conflicting chord is rejected and the existing binding remains unchanged. The core settings schema remains strict: an existing file that cannot parse required fields is a startup error. Additive navigation shortcut fields are the explicit exception: missing `back` and `forward` fields use their field-level defaults so older settings files remain readable.

The application icon is maintained as `assets/icons/yazi-gui.svg` and a multi-size `assets/icons/yazi-gui.ico`. `build.rs` embeds resource ID 1 with `embed-resource`; GPUI uses that resource for the window class and the native tray bridge loads the same ID.
