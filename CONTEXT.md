# yazi-gui Domain Language

This glossary defines the user-facing concepts used by the graphical file manager and keeps navigation and file-operation terms consistent.

## Navigation

**Active Tab**:
The tab whose directory, selection, and preview are currently shown.
_Avoid_: current window, selected page

**Workspace State**:
The runtime state coordinated by the main window, including GUI tabs, shared file-operation state, settings, and tray state.
_Avoid_: global model, window cache

**GUI Tab**:
An independent local file view with its own directory snapshot, selection, sorting, search session, preview, and asynchronous request state.
_Avoid_: yazi tab, process tab

**Tab Strip**:
The horizontal workspace control that shows GUI Tabs and opens new tabs. It shows all tabs followed by the new-tab control while they fit; after overflow, it shows the left navigation control, visible tabs, the right navigation control, and then the new-tab control.
_Avoid_: yazi tab bar, browser chrome

**Tab Viewport**:
The contiguous visible window of the Tab Strip after overflow. It keeps the active tab visible, shifts one tab at a time with the conditional side controls, and hides older tabs from the leading edge when the minimum tab width is reached.
_Avoid_: tab cache, hidden tab list

**Yazi Session**:
The single yazi backend process shared by GUI tabs. GUI tabs are local views over this session and accept events only for their current directory.
_Avoid_: one process per tab, protocol bridge state

**Operation Token**:
A per-view marker attached to an asynchronous scan or refresh request. A result whose token is no longer current is ignored.
_Avoid_: retry counter, progress percentage

**Address Path**:
The directory path used to identify and navigate the active tab's location.
_Avoid_: URL, location string

**Computer View**:
The virtual top-level view that lists available drive roots before entering a directory.
_Avoid_: desktop, root directory

**Navigation History**:
The per-GUI-Tab, in-memory sequence of real directories and Computer View destinations used by the Back and Forward commands during the current application run.
_Avoid_: recent folders, Favorites Bar

**History Entry**:
A single destination in a Navigation History. It is either a real directory or Computer View; consecutive equivalent directory paths share one entry.
_Avoid_: navigation request, refresh snapshot

**Folder Tree**:
A hierarchical navigation view of real folders below Computer View, drive roots, and their child folders.
_Avoid_: file tree, global folder index

**Favorite Folder**:
A user-pinned real folder path, including a drive root or UNC folder, that can be opened from the Favorites Bar.
_Avoid_: bookmark, shortcut

**Favorites Bar**:
The horizontal list of Favorite Folders below the Address Path, kept in the order the user added them.
_Avoid_: recent folders, history

**Sort State**:
The field and direction used to order entries in an active tab's file list.
_Avoid_: yazi sort mode, list preference

**Preview**:
The content summary shown for the currently selected file or directory.
_Avoid_: detail pane, inspector

**Preview Panel**:
The optional right-side surface that displays the Preview for the active selection.
_Avoid_: inspector pane, details sidebar

**Resizable Pane**:
A visible boundary whose drag changes the width of the Folder Tree, file list columns, or Preview Panel for the current session.
_Avoid_: persisted layout profile, responsive breakpoint

**File Column Layout**:
The shared Name, Modified, and Size column widths used by both the list header and every file row. The three columns stay inside one file-list horizontal viewport; its bottom scrollbar moves the header and rows together, while the ordinary wheel remains vertical.
_Avoid_: independent row alignment, floating metadata

**Application Titlebar**:
The custom top strip containing the SVG application identity, Settings control, native window controls, and the Windows non-client drag bridge.
_Avoid_: toolbar, external window chrome

**Command Tooltip**:
The localized label shown when the pointer rests on an icon-only command button.
_Avoid_: hidden command name, text toolbar label

**File Search Field**:
The always-visible file-page input that searches the active real directory by name or relative path.
_Avoid_: address input

**Refresh Operation**:
The active-tab action that sends yazi's `cd <current directory>` action asynchronously, completes when yazi accepts the command, and applies the resulting `gui-files` snapshot when it arrives; in Computer View it rescans drive roots in the background.
_Avoid_: reload protocol, global refresh

**Refresh Request**:
A per-tab in-flight refresh marker tied to the requested directory. It prevents duplicate refreshes and clears when the command succeeds or reports an error; the later `gui-files` event updates the list independently.
_Avoid_: permanent loading state, synchronous reload

**Settings Page**:
The in-window page for theme, language, shortcuts, autostart, about information, and manual release updates.
_Avoid_: preferences dialog, external settings window

**Update Check**:
A manual request for the latest stable Windows release that matches the current package type.
_Avoid_: startup polling, arbitrary update source

**Update Download**:
A cancellable transfer of the matching release package that is accepted only after its published checksum is verified.
_Avoid_: unverified installer, background update

**System Proxy**:
The Windows proxy behavior used by update requests, including automatic configuration selected by the current user or system.
_Avoid_: app-specific proxy setting, direct-only request

**Restart Update**:
The explicit action that closes the current GUI, applies a verified package, and starts the updated version.
_Avoid_: silent update, self-overwrite

**Tray State**:
The Windows notification-area icon that restores the window on left click and exposes show, settings, and exit on the context menu.
_Avoid_: background service, hidden process

**Search Session**:
The recursive result state created by the File Search Field for the active real directory. Its query, generation, and results are cleared when the field is emptied or the user navigates or switches tabs.
_Avoid_: global index, system-wide search

**Search Result**:
A relative path returned by a Search Session. It keeps the existing file-row behavior: directories navigate into their real path and files use the normal open action.
_Avoid_: virtual file, copied path

## File Operations

**File Operation**:
A background filesystem action such as search, copy, delete, or directory scan with an explicit result and user-visible status.
_Avoid_: service task, command queue

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
