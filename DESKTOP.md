# Tauri 桌面版与外部 AI 操作

桌面版使用 Tauri 2；画布、字体和界面随应用离线打包。外部 AI 通过应用内置 MCP 与原生 stdio 连接器操作指定项目与画布，无需配置应用内的 AI API Key。

## 本地开发和打包

需要 Bun、Rust >= 1.89，以及平台构建工具（macOS 上为 Xcode Command Line Tools）。

```sh
bun install --frozen-lockfile
bun run desktop:dev
bun run desktop:build
```

macOS 产物位于 `src-tauri/target/release/bundle/macos/` 和 `bundle/dmg/`。此版本在 Apple Silicon macOS 上构建验证；Windows/Linux 需要各自平台构建，并覆盖 `bundle.targets`。Rust 与前端 Tauri 包保持相同 major/minor；提交的 Cargo.lock 固定到兼容本机 Rust 的 Tauri 2.11 系列。

Web 模式继续使用 `bun run dev` / `bun run build`，浏览器模式不启用桌面桥接。

## 安装到本机 AI 工具

桌面端 0.5.0 内置 MCP 服务。打开应用即启动服务，关闭或退出应用即停止，不依赖 Node.js、Codex 或其他开发工具。服务只有桌面进程一个生命周期；开发工具运行的原生 stdio 连接器只转发请求，桌面会话结束后自动退出，不会自行启动应用。

在「设置 → 安装 AI 工具」勾选 Codex、Claude Code、Pi、Cursor，再点击「安装 / 更新」。分别检测 MCP 与 Skill 的缺失、最新、待更新及异常状态。只修改勾选工具的 AI Excalidraw 条目和托管 Skill 文件；保留其他服务、偏好和权限设置，修改前产生私有备份，失败回滚。配置内容损坏、目标为符号链接等情况会停止并提示。

首次发现工具缺少配置时仅提示一次。安装、跳过或关闭后都不再自动弹出，应用升级不会重置记录；后续手动从设置进入。没有安装开发工具也不影响 MCP 运行。

未识别的工具可点击按钮旁「安装指南 → 一键复制给 AI」，把包含本机实际路径、原生连接配置、配套 Skill 目录的指令交给该工具处理。此动作只导出应用自己的 Skill，不修改开发工具配置。让工具查明自身配置格式、备份并合并设置；不需要安装 Node 或复制连接令牌。

手动配置示例（请替换为本机实际应用位置）：

```json
{"mcpServers":{"ai-excalidraw":{"type":"stdio","command":"/Applications/AI Excalidraw.app/Contents/MacOS/ai-excalidraw-desktop","args":["--mcp-stdio"]}}}
```

客户端重新加载 MCP 或重开会话后加载 Skill。桌面重启后需客户端重新连接；连接文件包含的每次启动随机令牌由连接器自行读取，不写入开发工具配置。开发兼容脚本 `scripts/mcp-server.mjs` 是可选 Node 转发器，不是独立服务。推荐安装器配置的原生连接器。

配置依据：[Codex MCP](https://developers.openai.com/codex/mcp/)、[Claude Code MCP](https://code.claude.com/docs/en/mcp)、[Cursor MCP](https://cursor.com/docs/mcp)、[Pi MCP](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/mcp.md)。本机 Pi 1.4.0 的真实配置加载器也已核验。

## 工具

| 工具 | 功能 |
|---|---|
| `get_canvas` | 完整元素、选区、包围盒、当前会话 ID |
| `add_elements` | 批量新建矩形、椭圆、菱形、文字、箭头和线条 |
| `update_elements` | 按 ID 修改位置、大小、文字、颜色等 |
| `delete_elements` | 删除指定元素及绑定文字 |
| `fit_canvas` | 缩放定位到全部或指定图形 |
| `undo` / `redo` | 使用编辑器自身历史撤销或重做，并保存结果 |

新增元素需要 `id/type/x/y/width/height`；ID 不能与已有内容冲突。文本作为单独的 `text` 元素，指定 `text` 和合适的宽高。箭头 `points` 为相对 `x/y` 的坐标数组。更改元素类型需要删除后重新创建。批量操作最多 500 项。

修改会立即应用并进入撤销历史；可点击画布的“撤销”按钮，或让 AI 调用 `undo` / `redo`。MCP 修改在回复成功前通过 Rust 原子写入本地画布文件并同步落盘，写入失败会明确报错。启动时先恢复原生画布文件，再加载编辑器。操作前先读取画布，避免修改用户刚切换的其他会话。上游的本地会话存储和备份功能仍保留。超时后先读取场景再重试，以免重复操作。

## 本地连接机制

原生桥接仅监听 `127.0.0.1` 的随机端口，不提供公网入口。每次应用启动生成随机令牌，画布监听器就绪后才写入应用数据目录的 `bridge.json`（macOS/Linux 权限 0600）。原生连接器启动时读取端口和令牌；应用重启后重新连接即可，无需重新安装配置。应用只允许运行一个实例；重复打开会聚焦现有画布。浏览器 Origin 请求被拒绝；无令牌请求被拒绝。不提供 Shell、任意文件读取或系统控制工具。

macOS 的连接信息默认位于 `~/Library/Application Support/com.local.ai-excalidraw/bridge.json`。可通过 `AI_EXCALIDRAW_BRIDGE_FILE` 为 MCP 服务指定其他连接信息文件。画布内容只在执行工具时通过本地接口提供给连接的 AI 客户端。

原生画布文件位于同一应用数据目录的 `canvases/` 下，按会话分别保存，文件权限为 0600。桌面手工编辑也会随原有防抖写入同步保存原生快照；MCP 修改额外等待原生文件保存完成。

应用内原有 AI 聊天仍支持 OpenAI 兼容接口；桌面版使用 Tauri HTTP 插件，支持 HTTPS 服务与本机 HTTP 服务（localhost/127.0.0.1），避免 WebView 跨域问题。API 配置仍遵循上游的 localStorage/备份逻辑。桌面首次启动不强制弹出 API 设置，外部 MCP 操作无需配置该密钥。macOS 构建使用本地 ad-hoc 签名，未使用 Developer ID 证书或进行苹果公证。

## 验证

```sh
bun run lint
bun run test
bun run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --release --all-targets --locked
cargo test --manifest-path src-tauri/Cargo.toml --release --locked
# 先启动真实桌面应用；本测试临时创建并清理自有图形
node scripts/smoke-mcp.mjs
```

参考：[Tauri 2](https://v2.tauri.app/)、[HTTP 插件](https://v2.tauri.app/plugin/http-client/)、[Excalidraw 本地字体](https://docs.excalidraw.com/docs/@excalidraw/excalidraw/installation)、[Codex MCP](https://developers.openai.com/codex/mcp)。

## 本地项目与多画布（0.2.0）

点击“打开本地项目”选择一个文件夹。文件夹即项目，`.excalidraw` 文件即画布，子目录用于分类。左侧文件树支持新建画布、创建目录、搜索、导入、重命名/移动、复制、在 Finder 中显示和移到废纸篓。其他类型文件保持原样，不显示在画布列表中；删除目录时其全部内容一并进入废纸篓。

画布通过标签页切换。修改约 650ms 后自动保存，可用 ⌘S 立即保存；⌘O 打开已有文件，⌘⇧O 打开项目。系统中“打开方式 → AI Excalidraw”也可以打开 `.excalidraw`。直接打开项目之外的文件会将其父目录登记为项目。

原始画布使用 Excalidraw 自身序列化接口，包含图形、画布设置和内嵌图片。项目辅助目录 `.ai-excalidraw/project.json` 保存各文件的 AI 对话；应用数据目录中的 `workspace.json` 记录最近项目及打开标签页，不承载图形内容。

程序每次保存前比较磁盘版本，防止覆盖外部修改；窗口重新获得焦点和每 8 秒检查一次外部文件变化。冲突提示允许“另存副本”保留本地编辑，或确认后“重新加载文件”。关闭窗口和退出前会等待保存；失败时保留窗口，处理错误后再退出。强制结束进程无法执行退出前保存。

旧版缓存不会自动删除。打开项目后的空白工作区提供“将旧版缓存画布迁移到当前项目”，将非空历史画布复制为独立文件。可关闭标签页返回此入口。

外部 AI 先 `get_workspace` / `list_projects` / `list_files`，再 `open_canvas` 获得 `canvasId`。`add_elements`、`update_elements`、`delete_elements`、`fit_canvas`、`undo`、`redo` 必须显式携带 `canvasId`。已经打开的后台标签可通过 ID 编辑，操作完成前等待该文件落盘；画布重命名后需要重新获取 ID。`get_canvas` 可省略 ID 读取当前标签。项目管理另有 `open_project`、`create_canvas`、`create_directory`、`rename_file`、`duplicate_canvas`、`delete_file`、`reveal_file`。

项目路径采用本地真实目录，画布和目录参数采用项目内相对路径。拒绝 `..`、绝对路径、隐藏辅助目录和符号链接穿越。此发行包面向 Apple Silicon macOS；本地选择器、Finder 与废纸篓集成使用 macOS 系统能力。

文件树跳过隐藏目录、符号链接以及 `node_modules`、`target`，避免项目包含依赖或编译目录时反复扫描大量无关文件。每 8 秒仅检查已打开文件，文件树在获得焦点、显式刷新或项目操作后更新。

## 文件夹演示与临时批注（0.3.0）

- 文件夹右键/省略号 →「播放此文件夹」；顶部「播放」打开当前画布所在目录。根目录同样可播放。
- 一份 `.excalidraw` 文件是一页。准备窗口提供真实缩略图、自然文件名排序、拖动/上下箭头调整顺序、包含子文件夹和保存顺序。可从头或选中页开始播放。
- 先在编辑画布框选课件主体，再点「播放」旁的下拉箭头 →「将选区设为播放区域」。区域四周留16画布单位防止粗糙描边裁切；可恢复播放整张画布。
- 播放使用独立 SVG 课件快照，不连接原画布编辑器/自动保存。图片、字体和课件背景随预览导出；按原始比例适配屏幕。演示开始前先保存编辑内容；准备失败保留窗口并展示错误。
- 原生全屏播放：←/→、PageUp/PageDown、空格/Shift空格翻页，Home/End跳首尾，缩略图直接跳页，Esc结束并返回原编辑位置。工具栏有焦点时，空格保留按钮激活语义。
- P指针、B画笔、H荧光笔、E橡皮擦；四种颜色；⌘/Ctrl-Z撤销、Shift-Z重做。一次落笔或擦除手势是一条历史记录。橡皮只作用于临时批注。
- 批注按页保存在内存，翻页仍在；「清空本页」可撤销，「清空全部」需确认。退出播放/退出应用后全部丢弃，再次播放为空白批注。课件文件不写入批注。
- `.ai-excalidraw/project.json` 的 `presentation` 保存文件夹顺序/是否包含子文件夹，以及画布播放矩形；重命名/移动会同步路径，删除会清理关联，复制画布继承播放区域。
- 0.3.0 时外部AI共支持20个MCP工具，新增 `get_presentation {projectId}` 与 `set_presentation {projectId,presentation}`。仅管理演示配置；全屏播放和批注由用户在应用内控制。播放时项目/画布写入锁定，`get_workspace.presenting` 可查询状态。
- macOS最小窗口800px。web上游编辑器和非macOS原生打包不在本轮发行验证范围。

## AI 对话与本机工具

聊天输入框上方和设置中均可选择「对话来源」：模型 API、Codex、Claude Code、Cursor 或 Pi。模型 API 沿用原有 API Key、Base URL 和模型设置；本机工具复用各自的登录与模型配置，需要先安装对应 CLI 并在终端完成登录。Cursor 需要 `cursor-agent` 或 `agent`，仅安装编辑器启动命令 `cursor` 不足以在应用内对话。

本机工具使用自己的 Agent 循环执行工具调用。应用分别展示助手正文、可展开的思考和工具执行记录，并随画布对话保存。点击停止或切换会话会取消对应请求。Codex 优先使用 app-server，并按项目、画布与会话恢复原生 thread；其他 CLI 或兼容路径携带完整对话上下文并保留自己的 Agent 循环，实际恢复能力以运行记录为准。正文中的元素 JSON 可继续绘图，思考和工具结果不会被当作新图形。

打开项目后，读取和安装优先使用当前画布所属项目根目录的 `.agents/mcp.json`（`mcpServers` JSON 对象）与 `.agents/skills/`。设置中的安装页和安装指南会显示并使用同一项目范围；项目内工具共用配套 MCP 配置和 `ai-excalidraw-desktop` Skill。未选择项目时使用各工具用户目录的配置。安装保留其他配置并创建备份；配置损坏时不会重置整个文件。

应用内适配器负责将项目 `.agents` 内容提供给对应 CLI，并保留它自己的登录配置。单独从终端启动外部工具时，需遵循该工具的原生配置或插件加载方式，不能假定所有 CLI 都会自动发现通用 `.agents/mcp.json`。工具自身已有的权限限制仍然生效。

若工具原生配置与项目配置存在该 CLI 无法覆盖的同名传输或启用策略冲突，启动前会提示具体处理方式，不会静默采用错误配置。Pi 的等效同名配置可以共存；项目 MCP 接入需要支持 `registerMcpServer` 的版本。

本机对话中的画布 MCP 操作绑定发送请求时的 `canvasId`；生成中可以读取和编辑该画布，其他画布操作和项目导航继续受工作区保护。原生桥接串行处理操作，并行工具调用遇到忙碌响应时应先读取状态再重试。

## 可恢复 Agent 与项目文件（0.5.0）

运行记录统一保存执行过程、计划、真实用户输入、操作回执和结果核验。中断后可核对已保存的修改并继续，恢复记录不会再次执行写入。项目 Skill 包支持源文件预览、资源、版本冲突检查、原子保存和备份；项目记忆需用户确认具体内容后才写入。画布支持本轮平台修改的局部回退，拒绝覆盖后续用户编辑。直接绘图和选中元素的原 ID 修改继续保留。

当前 MCP 共 26 个工具，新增项目资源清单、受限文件预览、Skill 包保存、真实人工输入、确认项目记忆和执行计划工具；写入和人工交互需要绑定项目的应用内任务。内置画布 Skill 为 1.5.0。各后端能力、日志和权限边界见 [Agent 架构说明](./docs/agent-architecture.md)。
