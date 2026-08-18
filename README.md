# yazi-gui

[English](#english) · [中文](#中文)

## English

`yazi-gui` is a native Windows GUI file manager built with [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui) and backed by [yazi](https://github.com/sxyazi/yazi). It provides a graphical workflow while keeping yazi responsible for directory state and navigation events.

### Features

- Multi-tab navigation, directory-first listings, and sorting by name, modified time, or size.
- Address navigation, recursive search, and inline filename editing for rename/create operations.
- File previews, syntax highlighting, image previews, and local-time modified timestamps.
- Resizable folder tree, file columns, and preview panel with aligned name, modified, and size data.
- Win11-style custom titlebar and icon-only file commands with localized hover tooltips.
- Copy, cut, and paste with byte-level copy progress and block-boundary cancellation.
- Computer View for drives, refresh, Windows Recycle Bin handling, and confirmation-based permanent deletion on network drives.
- Theme, language, shortcut, autostart, tray, about, and manual update settings.

### Requirements

- Windows with a Rust Edition 2024 toolchain.
- The GUI-only package requires `yazi` and `ya` installed and available on `PATH`.
- The bundled package includes yazi/ya `26.8.15` for Windows x64.

### Getting started

1. Run `cargo run` for a development build; it starts in the current working directory.
2. Run `cargo build --release` to create `target/release/yazi-gui.exe`.

To run the bundled build locally, build it and explicitly stage the fixed yazi package beside the executable:

```powershell
cargo build --release --features bundled-yazi
.\scripts\stage-bundled-yazi.ps1 -YaziDirectory "path\to\yazi-26.8.15"
```

The staging command requires `yazi.exe` and `ya.exe` version `26.8.15`; the bundled executable does not use `PATH` at runtime.

Useful checks:

```powershell
cargo fmt --all
cargo check
cargo test
```

### Release packages

Each `v*` tag creates two Windows x64 archives: the GUI-only package, which uses yazi/ya from `PATH`, and a bundled package containing yazi/ya `26.8.15`. Both packages include the GUI's `assets/yazi` configuration. Release archives are published with a `SHA256SUMS.txt` file.

Settings are stored in `%APPDATA%\yazi-gui\settings.json`, with autostart disabled by default. Runtime yazi configuration lives under `assets/yazi/`; the `gui-files.yazi` plugin publishes directory snapshots consumed by the GUI.

The Settings page can check the latest stable GitHub Release for the matching package, download it through the Windows system proxy with byte progress and cancellation, verify its SHA-256 checksum, and offer a restart to apply it.

### Project layout

- `src/main.rs` — GPUI UI, tabs, selection, previews, sorting, search, and file operations.
- `src/yazi.rs` — yazi subprocess bridge and event parsing.
- `src/settings.rs` / `src/tray.rs` — settings persistence and the Windows tray bridge.
- `src/update.rs` — GitHub release checks, proxy-aware downloads, checksum verification, and restart updates.
- `scripts/stage-bundled-yazi.ps1` — explicit local staging for a bundled release build.
- `assets/yazi/` — bundled yazi configuration and GUI event plugin.
- `assets/icons/` — SVG/ICO application icon sources.
- `docs/adr/` — architecture decision records.

## 中文

`yazi-gui` 是一个基于 [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui) 构建、由 [yazi](https://github.com/sxyazi/yazi) 驱动的原生 Windows 图形文件管理器。GUI 负责交互和展示，yazi 负责目录状态与导航事件。

### 功能

- 多标签页、目录优先列表、按名称/修改时间/大小排序。
- 地址栏跳转、递归文件搜索、文件名行内重命名与新建。
- 文件预览、语法高亮、图片预览，以及本地时间格式的修改时间。
- 文件夹树、文件列和预览面板可拖拽调整宽度，名称/修改时间/大小保持对齐。
- Win11 风格自定义标题栏，文件操作按钮使用图标并在悬停时显示本地化提示。
- 复制/剪切/粘贴；复制显示字节级进度，可在块边界取消。
- “此电脑”磁盘视图、刷新、Windows 回收站和网络驱动器永久删除确认。
- 主题、语言、快捷键、自启动、托盘、关于和手动更新设置。

### 环境要求

- Windows，Rust Edition 2024 工具链。
- GUI-only 包需要已安装并加入 `PATH` 的 `yazi` 和 `ya`。
- bundled 包内置 Windows x64 版本的 yazi/ya `26.8.15`。

### 快速开始

1. 运行 `cargo run` 启动开发版本，程序从当前工作目录开始。
2. 运行 `cargo build --release` 构建 `target/release/yazi-gui.exe`。

本地运行 bundled 版本时，需要先构建并将固定版本的 yazi 包显式放到可执行文件旁：

```powershell
cargo build --release --features bundled-yazi
.\scripts\stage-bundled-yazi.ps1 -YaziDirectory "path\to\yazi-26.8.15"
```

staging 命令要求 `yazi.exe` 和 `ya.exe` 均为 `26.8.15`；bundled 可执行文件运行时不会使用 `PATH`。

常用验证命令：

```powershell
cargo fmt --all
cargo check
cargo test
```

### Release 包

每个 `v*` tag 会创建两个 Windows x64 压缩包：GUI-only 包使用 `PATH` 中的 yazi/ya，bundled 包内置 yazi/ya `26.8.15`。两个包都包含 GUI 使用的 `assets/yazi` 配置，并附带 `SHA256SUMS.txt` 校验文件。

设置保存在 `%APPDATA%\yazi-gui\settings.json`；自启动默认关闭。运行时 yazi 配置位于 `assets/yazi/`，其中的 `gui-files.yazi` 插件负责向 GUI 发布目录快照。

设置页可以检查与当前包匹配的最新稳定版 GitHub Release，使用 Windows 系统代理下载并显示字节进度、支持取消，校验 SHA-256 后提供重启更新。

### 项目结构

- `src/main.rs` — GPUI 界面、标签页、选择、预览、排序、搜索和文件操作。
- `src/yazi.rs` — yazi 子进程桥接和事件解析。
- `src/settings.rs` / `src/tray.rs` — 设置持久化和 Windows 托盘桥接。
- `src/update.rs` — GitHub Release 检查、系统代理下载、校验和重启更新。
- `scripts/stage-bundled-yazi.ps1` — bundled release 本地显式 staging。
- `assets/yazi/` — 内置 yazi 配置与 GUI 事件插件。
- `assets/icons/` — SVG/ICO 应用图标源文件。
- `docs/adr/` — 架构决策记录。
