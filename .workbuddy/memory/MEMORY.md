# yazi GUI 项目长期笔记

## 项目定位
为 yazi 终端文件管理器做一个原生 GUI。目标：跨平台（Windows/macOS/Linux）。

## 技术栈（已定）
- **UI 框架**：GPUI 0.2.2（crates.io，Zed 编辑器同款 GPU 加速框架）
- **后端引擎**：yazi（本机已装 26.8.15），**无需 PTY**，用 `std::process::Command` 分离管道启动，stdout 即纯净 DDS 事件流
- **语言/工具链**：Rust 1.97.1 + MSVC Build Tools 14.44（Windows）

## 架构（三层）
1. **GPUI 前端**：文件列表（虚拟滚动）、预览窗格、标签栏/状态栏
2. **Rust 桥接层**：进程管理（无 PTY 管道启动 yazi）、事件解析器（--local-events）、动作发送器（ya emit-to）
3. **yazi 引擎**：文件操作、Lua 插件预览、git/排序/搜索

## 关键约束与方案（已确认）
- `--client-id` 与 `ya emit-to` 的 receiver **必须是纯数字**（u64）。
- 启动：`yazi --client-id <ts> --local-events cd,hover,rename,trash,delete,move,bulk`；发指令：`ya emit-to <client_id> cd <path>`。
- 事件格式：`kind,receiver,sender,{json}`；cd/hover 的 body 是 `{"tab":N,"url":"..."}`。
- `--local-events` 只推增量事件，不推完整文件列表 → 写 yazi Lua 插件用 `ps.pub` 发布完整目录（路径/类型/大小/mtime/选中态）。

## GPUI 0.2.2 关键 API（与旧版不同，勿照抄老文档）
```rust
impl Render for Root {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement { ... }
}
// 窗口：window_bounds: Some(WindowBounds::centered(size(px(w), px(h)), cx))
// 打开：cx.open_window(opts, |_window, cx| cx.new(|_cx| Root))
```

## 环境
- cargo/rustc 位于 `~/.cargo/bin`（已加 PATH）；真实二进制在 `~/.rustup/toolchains/stable-x86_64-pc-windows-msvc/bin/`
- 沙箱环境跑 rustup 会致代理硬链接失败，需手动重建（见 2026-08-17.md）
