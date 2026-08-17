# Repository Guidelines

## Project Structure & Module Organization

- `src/main.rs` contains the GPUI application, tabs, selection, previews, themes, and file operations.
- `src/yazi.rs` owns the yazi subprocess bridge, event parsing, and `ya emit-to` commands.
- `assets/yazi/` contains the runtime yazi configuration and the `gui-files.yazi` plugin that publishes directory state.
- `examples/` contains standalone probes for syntax highlighting, trash behavior, and yazi pipes/operations.
- `target/` is generated build output and should remain uncommitted.

## Build, Test, and Development Commands

Install Rust with edition 2024 support, and make both `yazi` and `ya` available on the Windows `PATH`.

- `cargo fmt --all` — format Rust sources.
- `cargo check` — compile-check the crate quickly.
- `cargo build` — create a debug build.
- `cargo run` — launch the GUI. Set `START_DIR` in `src/main.rs` to a valid local directory before running.
- `cargo test` — run the test harness; the repository currently has no committed automated tests.
- `cargo run --example highlight_probe` — verify syntect setup. The yazi probes require a working yazi installation and use local paths.

## Coding Style & Naming Conventions

Use standard `rustfmt` formatting with four-space indentation. Follow Rust naming conventions: `snake_case` for functions and modules, `CamelCase` for types, and `SCREAMING_SNAKE_CASE` for constants. Keep yazi protocol/process code in `src/yazi.rs`; keep UI state and rendering in `src/main.rs`. Prefer `Result`-based error handling for subprocess and filesystem operations, and run `cargo fmt --all` before committing.

## Engineering Principles

- Do not preserve backward compatibility. Remove obsolete paths instead of adding compatibility layers, fallbacks, or migrations.

- Choose the simplest implementation that fully meets the current requirements. Avoid speculative abstractions, configuration, and indirection.

- Grow the system in layers. Start from the smallest version that works end to end, and add each new capability on top of a product that already works. Never trade a working product for unfinished complexity.

- Keep components modular and concerns clearly separated.

- Prefer established, well-maintained libraries when they reduce overall complexity or improve reliability. Do not reimplement common functionality without a clear reason.

- Lean on the dependencies already in the project before writing your own implementation or adding packages. Do not assume a library lacks a capability without checking its documentation and types.

- Make architectural decisions for the long term. Do not accept a stopgap that only works for now and is meant to be replaced later.

- Study how established products solve the problem before designing a solution. Adopt their proven patterns and conventions rather than inventing an approach from scratch.

## Testing Guidelines

There is no configured coverage threshold or separate `tests/` directory. Add focused `#[test]` cases for parser and filesystem helpers, using names such as `parse_gui_files_event`. Manually exercise navigation, tabs, selection, context-menu operations, previews, and theme switching through `cargo run`; use the yazi probes when changing process or event handling.

## Commit & Pull Request Guidelines

Recent commits use concise Conventional Commit-style prefixes such as `feat:` and `fix:` (summaries may be in Chinese). Keep commits focused and describe the user-visible behavior. Pull requests should explain the change, list validation commands, note yazi/Windows setup requirements, link an issue when applicable, and include screenshots or a short recording for UI changes. Do not include generated output or machine-specific test artifacts.
