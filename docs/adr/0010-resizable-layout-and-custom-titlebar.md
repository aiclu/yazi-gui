# Resizable layout and custom titlebar

## Status

Accepted

## Decisions

The file page uses one shared File Column Layout for headers and rows. The Folder Tree, each file-list column, and the Preview Panel expose visible drag boundaries with practical minimum and maximum widths. The header and rows share a file-list-only horizontal viewport with a bottom drag scrollbar; its scroll handle is axis-restricted so a normal wheel stays vertical. Widths are session-only; they are not added to the strict settings file because a useful layout must remain available without introducing a second persistence schema.

The application uses a custom Application Titlebar. It keeps the Settings control immediately before the native minimize, maximize, and close control areas, while the center sends the Windows `WM_NCLBUTTONDOWN`/`HTCAPTION` message pair to start a real native move. The titlebar SVG, taskbar resource, and tray icon share the same application icon source. File-page commands and navigation controls are icon-only and expose localized Command Tooltips on hover; settings forms and dialogs keep their text labels.

This keeps alignment and interaction ownership explicit: layout state stays in the GUI workspace, filesystem state stays in yazi/filesystem helpers, and window controls stay platform-native. The tradeoff is that pane widths reset when the application restarts.
