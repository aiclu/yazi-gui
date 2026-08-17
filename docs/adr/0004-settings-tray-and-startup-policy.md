# Keep settings in the main window and make startup opt-in

Settings use one JSON file under `%APPDATA%\yazi-gui\settings.json`. The settings page stays in the existing window and applies theme, language, and shortcut changes immediately. Shortcut editing covers file-management actions; native text-editing keys remain fixed so address-bar and rename input keep their platform behavior.

The Windows notification-area icon owns the hide-to-tray behavior: closing the window hides it, a left click restores it, and the context menu exposes show, settings, and exit. Exit is blocked while copy, delete, or refresh work is active and reports the reason in the status area. A native Windows Shell tray bridge is used because the current dependency cache does not contain a tray crate.

Autostart is explicitly disabled in the default settings. Enabling it writes one `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\yazi-gui` value that starts the executable with `--background`; disabling it removes that value. No startup registry value is written on first launch. The update entry is retained as a visible product surface but reports that no update source is configured, so this change does not perform a network check.
