# yazi-gui Domain Language

This glossary defines the user-facing concepts used by the graphical file manager and keeps navigation and file-operation terms consistent.

## Navigation

**Active Tab**:
The tab whose directory, selection, and preview are currently shown.
_Avoid_: current window, selected page

**Address Path**:
The directory path used to identify and navigate the active tab's location.
_Avoid_: URL, location string

**Computer View**:
The virtual top-level view that lists available drive roots before entering a directory.
_Avoid_: desktop, root directory

**Sort State**:
The field and direction used to order entries in an active tab's file list.
_Avoid_: yazi sort mode, list preference

**Preview**:
The content summary shown for the currently selected file or directory.
_Avoid_: detail pane, inspector

**Refresh Operation**:
The active-tab action that sends yazi's `cd <current directory>` action asynchronously, completes when yazi accepts the command, and applies the resulting `gui-files` snapshot when it arrives; in Computer View it rescans drive roots in the background.
_Avoid_: reload protocol, global refresh

**Refresh Request**:
A per-tab in-flight refresh marker tied to the requested directory. It prevents duplicate refreshes and clears when the command succeeds or reports an error; the later `gui-files` event updates the list independently.
_Avoid_: permanent loading state, synchronous reload

**Settings Page**:
The in-window page for theme, language, shortcuts, autostart, about information, and the explicitly unconfigured update entry.
_Avoid_: preferences dialog, external settings window

**Tray State**:
The Windows notification-area icon that restores the window on left click and exposes show, settings, and exit on the context menu.
_Avoid_: background service, hidden process

**Search Session**:
The transient recursive search mode for the active real directory. Its query, generation, and results are discarded when the user navigates, switches tabs, or presses Escape.
_Avoid_: global index, system-wide search

**Search Result**:
A relative path returned by a Search Session. It keeps the existing file-row behavior: directories navigate into their real path and files use the normal open action.
_Avoid_: virtual file, copied path

## File Operations

**File Clipboard**:
The set of selected filesystem entries held for a later copy or cut paste operation.
_Avoid_: text clipboard, selection buffer

**Pending Operation**:
A rename, creation, address navigation, search, or shortcut-recording action waiting for confirmation or cancellation.
_Avoid_: command mode, edit mode

**Inline Name Editor**:
The filename-row editor used for rename and new-item operations. Rename selects the stem while keeping the extension, and new items appear in a temporary unsorted row.
_Avoid_: modal name dialog, top input bar

**Shortcut Recorder**:
The click-to-record settings control that accepts one key chord, rejects conflicts, and lets Backspace clear a binding.
_Avoid_: shortcut text field, multi-stroke binding

**Icon Asset**:
The SVG master and multi-size Windows ICO compiled into resource ID 1 for both the GPUI window and notification-area icon.
_Avoid_: generic application icon, file-type icon

**Release Package**:
A Windows x64 zip produced from a `v*` tag. The GUI-only package uses yazi/ya from `PATH`; the bundled package uses the explicitly compiled-in bundled-yazi mode and ships yazi/ya 26.8.15 beside the GUI.
_Avoid_: source checkout, debug build

**Bundled Yazi**:
The fixed yazi 26.8.15 and ya 26.8.15 binaries included in the bundled Release package. The bundled build does not silently switch to a system executable.
_Avoid_: latest yazi, optional fallback binary

**Status Message**:
The latest user-facing result or error for a file operation, shown alongside the active view's item count.
_Avoid_: log line, notification toast

**Transfer Progress**:
The byte-level status of a file copy operation, including completed bytes, total bytes, and the current entry.
_Avoid_: copy status, loading message

**Transfer Cancellation**:
The user's request to stop an active copy transfer at a read/write boundary and remove its unfinished temporary file.
_Avoid_: undo, rollback

**Network Path**:
A UNC path or a drive-letter path whose Windows drive type is remote.
_Avoid_: shared folder string, remote URL

**Permanent Delete**:
Direct filesystem removal used for Network Paths because they do not provide the local Recycle Bin flow.
_Avoid_: trash fallback, hard delete mode

**Delete Confirmation**:
The blocking dialog shown before a batch containing Network Paths is deleted; local items still go to the Recycle Bin after confirmation.
_Avoid_: warning toast, second-click delete

**Autostart**:
The opt-in Windows Run entry. Its default is disabled and the first launch does not write a startup value.
_Avoid_: implicit startup, scheduled task
