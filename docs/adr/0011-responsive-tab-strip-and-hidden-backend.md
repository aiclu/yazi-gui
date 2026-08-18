# Responsive Tab Strip and Hidden Backend

## Context

The GUI can keep several independent tabs while the custom titlebar and file page use a compact Windows layout. Rendering every tab at its content width makes the strip overflow unpredictably and leaves no direct way to reach tabs hidden by the window boundary. Starting yazi with ordinary Windows process creation also allows a console window to appear beside the GUI.

## Decision

The Tab Strip uses the compact layout `[all tabs][+]` while every tab fits. Once the minimum tab width would be exceeded, it switches to `[left][visible tabs][right][+]`: the navigation controls are conditional, and the new-tab control follows the right control instead of being pushed to the window edge. Tabs keep a default width while there is room; when the strip is nearly full, all visible tabs share the remaining width equally down to the minimum. Additional tabs extend the Tab Viewport rather than shrinking existing tabs below that minimum. The left and right controls move the viewport one tab at a time, and switching or creating a tab makes the active tab visible. Older tabs leave the leading edge first.

The shared Yazi Session remains the backend for GUI tabs, but Windows starts both `yazi` and `ya` with `CREATE_NO_WINDOW`, and the GUI executable uses the Windows GUI subsystem. The GUI embeds the SVG Icon Asset in its GPUI asset source, with an explicit size, shrink policy, and theme color, so the custom titlebar does not depend on a filesystem path at runtime.

## Consequences

The strip has deterministic minimum sizing and predictable overflow behavior at narrow window sizes. Users can reach every tab without horizontal mouse-wheel behavior or a second tab list. Yazi continues to provide directory events without opening an extra console window, and the titlebar icon is available in debug, release, and bundled builds from the same source asset.
