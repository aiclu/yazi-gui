# yazi-gui

[English](#english) · [中文](#中文)

## English

`yazi-gui` is a native Windows GUI file manager built with [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui) and backed by [yazi](https://github.com/sxyazi/yazi). It provides a graphical workflow while keeping yazi responsible for directory state and navigation events.

### Features

- Multi-tab navigation, directory-first listings, and sorting by name, modified time, or size.
- Address navigation, recursive search, and inline filename editing for rename/create operations.
- File previews, syntax highlighting, image previews, and local-time modified timestamps.
- Copy, cut, and paste with byte-level copy progress and block-boundary cancellation.
- Computer View for drives, refresh, Windows Recycle Bin handling, and confirmation-based permanent deletion on network drives.
- Theme, language, shortcut, autostart, tray, and about settings.

### Requirements

- Windows with a Rust Edition 2024 toolchain.
- The GUI-only package requires `yazi` and `ya` installed and available on `PATH`.
- The bundled package includes yazi/ya `26.8.15` for Windows x64.

### Getting started

1. Run `cargo run` for a development build; it starts in the current working directory.
2. Run `cargo build --release` to create `target/release/yazi-gui.exe`.

Useful checks:

```powershell
cargo fmt --all
cargo check
cargo test
```

### Release packages

Each `v*` tag creates two Windows x64 archives: the GUI-only package, which uses yazi/ya from `PATH`, and a bundled package containing yazi/ya `26.8.15`. Both packages include the GUI's `assets/yazi` configuration. Release archives are published with a `SHA256SUMS.txt` file.

Settings are stored in `%APPDATA%\yazi-gui\settings.json`, with autostart disabled by default. Runtime yazi configuration lives under `assets/yazi/`; the `gui-files.yazi` plugin publishes directory snapshots consumed by the GUI.

### Project layout

- `src/main.rs` — GPUI UI, tabs, selection, previews, sorting, search, and file operations.
- `src/yazi.rs` — yazi subprocess bridge and event parsing.
- `src/settings.rs` / `src/tray.rs` — settings persistence and the Windows tray bridge.
- `assets/yazi/` — bundled yazi configuration and GUI event plugin.
- `assets/icons/` — SVG/ICO application icon sources.
- `docs/adr/` — architecture decision records.

## 中文

`yazi-gui` 是一个基于 [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui) 构建、由 [yazi](https://github.com/sxyazi/yazi) 驱动的原生 Windows 图形文件管理器。GUI 负责交互和展示，yazi 负责目录状态与导航事件。

### 功能

- 多标签页、目录优先列表、按名称/修改时间/大小排序。
- 地址栏跳转、递归文件搜索、文件名行内重命名与新建。
- 文件预览、语法高亮、图片预览，以及本地时间格式的修改时间。
- 复制/剪切/粘贴；复制显示字节级进度，可在块边界取消。
- “此电脑”磁盘视图、刷新、Windows 回收站和网络驱动器永久删除确认。
- 主题、语言、快捷键、自启动、托盘和关于页面。

### 环境要求

- Windows，Rust Edition 2024 工具链。
- GUI-only 包需要已安装并加入 `PATH` 的 `yazi` 和 `ya`。
- bundled 包内置 Windows x64 版本的 yazi/ya `26.8.15`。

### 快速开始

1. 运行 `cargo run` 启动开发版本，程序从当前工作目录开始。
2. 运行 `cargo build --release` 构建 `target/release/yazi-gui.exe`。

常用验证命令：

```powershell
cargo fmt --all
cargo check
cargo test
```

### Release 包

每个 `v*` tag 会创建两个 Windows x64 压缩包：GUI-only 包使用 `PATH` 中的 yazi/ya，bundled 包内置 yazi/ya `26.8.15`。两个包都包含 GUI 使用的 `assets/yazi` 配置，并附带 `SHA256SUMS.txt` 校验文件。

设置保存在 `%APPDATA%\yazi-gui\settings.json`；自启动默认关闭。运行时 yazi 配置位于 `assets/yazi/`，其中的 `gui-files.yazi` 插件负责向 GUI 发布目录快照。

### 项目结构

- `src/main.rs` — GPUI 界面、标签页、选择、预览、排序、搜索和文件操作。
- `src/yazi.rs` — yazi 子进程桥接和事件解析。
- `src/settings.rs` / `src/tray.rs` — 设置持久化和 Windows 托盘桥接。
- `assets/yazi/` — 内置 yazi 配置与 GUI 事件插件。
- `assets/icons/` — SVG/ICO 应用图标源文件。
- `docs/adr/` — 架构决策记录。
