# gui_for_yazi 中文架构整改报告

## 结论

本项目的模块划分总体合理，但主窗口此前同时承担工作区状态、异步令牌、UI 行为、文件操作协调、更新流程和 Windows FFI，导致接口过宽、状态转换重复，且部分协议字段没有消费者。本次已按推荐方案完成全部整改，并保留现有用户可见行为。

## 已完成的整改

1. 工作区与 Yazi 接缝

   - 在 src/workspace.rs 增加 WorkspaceState，集中拥有 GUI 标签页、活动标签页、标签页视口、磁盘根目录、文件夹树以及刷新/搜索令牌。
   - 将旧的 Yazi client interface 收紧为 YaziSession 和目录导航 handle。
   - 删除 hover 事件、is_hidden、hovered 等未消费载荷，并同步精简 gui-files 插件。
   - 增加刷新、搜索过期结果的聚焦测试。

2. 文件操作

   - 合并目录扫描和递归搜索中的链接/目录分类逻辑。
   - 将复制取消状态和进度发送细节收回 fs_ops，主窗口只接收 TransferUpdate。
   - 删除重复的 operation_status，统一由 fs_ops 提供。
   - 保留单 worker、临时文件提交和取消时清理的既有行为。

3. UI 接口

   - 增加 UiProjection，文件页和设置页读取显示投影，而不是直接读取主窗口字段。
   - 增加 UiIntent，按钮、菜单、排序、标签页、窗口控制和文件交互通过统一意图进入主窗口。
   - 设置页的更新面板只消费更新状态投影。

4. 更新流程

   - 将 UpdatePhase、UpdateState 和请求生命周期操作迁移到 src/update.rs。
   - 检查、下载、取消、进度、完成和重启均校验 request token，旧结果不会覆盖新请求。
   - 增加更新生命周期测试。

5. Windows 平台适配器

   - 新增 src/platform/window.rs、tray.rs、registry.rs、filesystem.rs 和 http.rs。
   - 删除旧的 src/tray.rs，不保留兼容转发模块。
   - 主窗口、设置、文件操作和更新模块不再直接持有 Windows API 细节。

## unsafe 评估

无法安全删除全部 unsafe，因为以下行为本身依赖 Windows FFI：

- platform/window.rs：原生窗口句柄、窗口显示/移动/关闭。
- platform/tray.rs：通知区域窗口过程、菜单、图标和原始状态指针。
- platform/registry.rs：Windows Run 自启动项。
- platform/filesystem.rs：远程驱动器类型判断。
- platform/http.rs：WinHTTP 句柄和数据读取。

这些 unsafe 已集中到平台适配器；Yazi 的隐藏进程使用的是安全的 CommandExt interface，不需要新增 unsafe。

## 验证结果

- cargo fmt --all：通过。
- cargo check --all-features：通过。
- cargo test --all --quiet：39 个测试全部通过。
- git diff --check：通过。
- 换行检查：所有编辑文本文件均为统一 LF，无混合 CR/LF；本项目为 Rust/脚本项目，不适用 Visual Studio 后端 CRLF 强制规则。

## 设计记录

- [0012 Workspace State and Yazi Session Seam](adr/0012-workspace-state-and-yazi-session-seam.md)
- [0013 Windows Platform Adapters](adr/0013-windows-platform-adapters.md)
- [0014 Update Flow Lifecycle](adr/0014-update-flow-lifecycle.md)
