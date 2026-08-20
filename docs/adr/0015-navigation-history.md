# Per-tab navigation history

## Status

Accepted

## Context

GUI Tabs share one Yazi Session, but users expect each tab to retain its own browser-like directory navigation context. Directory changes can arrive asynchronously through Yazi events, while Computer View is a local virtual view. A history request must not be overtaken by another directory request or a tab switch.

## Decision

- Each GUI Tab owns an independent Navigation History. A new tab starts with Computer View as its first History Entry.
- History entries are real directory paths or Computer View. Directory equality uses the existing case-insensitive `tree_path_key`; consecutive equivalent destinations collapse into one entry.
- History is in memory only, is limited to 100 entries per tab, and is discarded when the application exits. A new navigation after going back truncates the forward branch.
- Computer View participates in the same Back and Forward sequence. Switching to it commits immediately after the local state changes; a real directory commits only after a matching `Cd` or `GuiFiles` event is received.
- The standard mouse navigation buttons are supported without configuration: `XBUTTON1`/GPUI `Navigate(Back)` goes back and `XBUTTON2`/GPUI `Navigate(Forward)` goes forward. The GUI consumes both press and release events; boundary and in-flight requests remain silent no-ops.
- Directory navigation is serialized. While one request is waiting, directory-entry controls and tab switching are ignored. Failed or stale-directory requests leave the current view and history cursor unchanged; non-navigation file operations remain available.
- The Yazi protocol and event set remain unchanged. The GUI coordinates history around the existing `cd`, `Cd`, and `GuiFiles` flow.

## Consequences

Navigation state is predictable across multiple tabs and supports toolbar and configurable keyboard commands without adding backend protocol concepts. Because history is session-scoped, restarting the application intentionally starts a new history. A real directory that disappears remains as a stale History Entry until a user attempts to revisit it; that failed attempt does not alter the entry or cursor.
