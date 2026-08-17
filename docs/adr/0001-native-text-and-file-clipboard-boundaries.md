# Use one native text editor and separate clipboard domains

yazi-gui uses one native GPUI text editor for address navigation, renaming, and creation so IME, caret, selection, and text shortcuts behave consistently. Text Ctrl+C/V/X stays in the operating-system text clipboard while the GUI-owned file clipboard handles selected filesystem entries when no text operation is pending; this follows established file-manager behavior without conflating text data and multi-file operations.
