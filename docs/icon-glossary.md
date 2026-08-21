# Icon glossary

This is the vocabulary for the local SVG icon set. Product code should use the `Icon` variant rather than a literal Unicode symbol.

| Icon | Meaning | Primary use |
| --- | --- | --- |
| `Add` | Add | New tab |
| `ArrowDown` / `ArrowUp` | Sort direction | File headers |
| `ArrowLeft` / `ArrowRight` | Back/forward movement | Navigation and tab scrolling |
| `Archive` | Compressed package | Archive files |
| `Binary` | Executable or binary | Executables and libraries |
| `ChevronDown` / `ChevronRight` | Expanded/collapsed | Folder tree and settings |
| `Clipboard` | Paste | Paste action |
| `Close` | Close/remove | Tabs, favorites, and window close |
| `Code` | Source or text | Code/text files |
| `Computer` | This computer | Computer view |
| `Copy` / `Cut` | Clipboard operations | File actions |
| `Edit` | Rename/edit | Rename action |
| `ExternalLink` | Open | Open selected item |
| `File` / `FilePlus` | Generic/new file | Generic files and create action |
| `Folder` / `FolderPlus` | Folder/new folder | Directories and create action |
| `Image` | Image file | Image files |
| `LayoutPanel` | Preview layout | Preview toggle |
| `Maximize` / `Minimize` | Window controls | Custom titlebar |
| `Refresh` | Refresh | Navigation toolbar |
| `Settings` | Settings | Custom titlebar |
| `Star` / `StarFilled` | Favorite state | Folder tree and current directory |
| `Trash` | Destructive delete | Delete action |

## Rendering rules

- Use 16px for file rows and toolbar controls, 14px for tree/favorite affordances, and 12–13px for compact sort/close controls.
- Use `theme.text` for normal icons, `theme.muted` for secondary affordances, `theme.blue` for selected/favorite states, and `theme.danger` only for destructive delete.
- Keep the icon's optical center aligned inside the existing 32px toolbar buttons; do not add per-call-site font-size or glyph offsets.
