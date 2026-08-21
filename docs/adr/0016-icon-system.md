# Unified icon system

## Status

Accepted

## Context

The GUI previously mixed a hand-authored application SVG, Unicode symbols, Windows/emoji glyphs, and file-type emoji. Their metrics and fallback fonts varied by platform, so identical controls did not share a visual language or reliable alignment.

GPUI renders SVGs as alpha masks tinted by the element text color. That makes local SVG assets a good fit for theme-aware interface icons, while the Windows taskbar and tray still need a fixed-color brand resource.

## Decisions

- Functional icons live as one SVG per icon under `assets/icons/ui/`.
- `src/ui/icons.rs` owns the `Icon` enum, asset mapping, and the only GPUI SVG renderer. UI call sites do not pass Unicode glyphs or emoji as icons.
- Functional SVGs use a 24×24 view box, a 1.75px rounded stroke, and transparent fills unless a filled state is semantically required.
- The application brand keeps the folder-and-spark motif but uses a refreshed geometric SVG. The titlebar uses that SVG while the Windows resource and tray bridge use a paired multi-size ICO with the same geometry; interface instances are theme-tinted by GPUI.
- File entries keep six categories: folder, image, archive, executable/binary, code/text, and generic file.
- The delete action uses the theme danger color; other interface icons inherit the normal text or muted color for their context.

## Consequences

The icon set is deterministic across Windows fonts and supports dark/light themes without an icon-font dependency. Adding an icon requires an SVG, an `Icon` variant, and an asset mapping entry, which keeps the inventory explicit and reviewable. The Windows `.ico` remains a separately generated distribution artifact because native resources cannot consume the GPUI asset source directly.

The visual reference board is [`docs/icon-system-preview.svg`](../icon-system-preview.svg), and the semantic inventory is [`docs/icon-glossary.md`](../icon-glossary.md).
