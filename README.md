# AI Excalidraw

[![CI](https://github.com/co-pine/ai-excalidraw/actions/workflows/ci.yml/badge.svg)](https://github.com/co-pine/ai-excalidraw/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE)
[![Bun](https://img.shields.io/badge/Bun-%E2%89%A51.0-fbf0df?logo=bun&logoColor=black)](https://bun.sh/)

用自然语言描述，让 AI 绘制手绘风格流程图、架构图、示意图。

Web 与本地桌面应用 · OpenAI 兼容 API 与本机 AI 工具 · 数据保存在本地

此 fork 在上游基础上新增 **Tauri 2 桌面应用、本地项目、多后端 Agent 与 MCP 画布操作**。当前桌面版为 **0.5.0**，开发、打包和连接方式见 [DESKTOP.md](./DESKTOP.md)，运行架构见 [Agent 架构说明](./docs/agent-architecture.md)。

---

## 功能特性

**绘图**

- 自然语言描述即可生成 Excalidraw 元素
- 流式响应，边生成边渲染到画布
- 选中元素后可让 AI 局部修改（同 id 更新）

**会话与工具**

- 多会话管理；新会话使用独立画布
- 多轮对话上下文（自动清洗历史中的 JSON / 思考内容）
- 工具调用：查询画布、删除元素、批量更新元素

**体验**

- 桌面端与移动端共用同一套绘图会话逻辑
- 中英文界面；暗色模式跟随 Excalidraw 主题
- 导入 / 导出本地备份（会话、画布、API 配置）

---

## 快速开始

需要 [Bun](https://bun.sh/) >= 1.0（推荐）或 Node.js >= 18。

```bash
git clone https://github.com/co-pine/ai-excalidraw.git
cd ai-excalidraw
bun install
bun run dev
```

打开 [http://localhost:5173](http://localhost:5173)。

| 命令 | 说明 |
|------|------|
| `bun run build` | 生产构建，产物在 `dist/` |
| `bun run preview` | 预览生产构建 |
| `bun run lint` | ESLint 检查 |
| `bun run test` | 单元测试 |

---

## 配置 AI API

首次启动会弹出设置对话框，也可通过环境变量配置（见 [`.env.example`](./.env.example)）：

| 配置项 | 说明 | 示例 |
|--------|------|------|
| API Key | API 密钥 | `sk-xxx` |
| Base URL | OpenAI 兼容地址 | `https://api.openai.com/v1` |
| Model | 模型名称 | `gpt-4o` |

**常用兼容服务**

| 服务 | Base URL |
|------|----------|
| OpenAI | `https://api.openai.com/v1` |
| 智谱 AI | `https://open.bigmodel.cn/api/paas/v4` |
| 阿里百炼 | `https://dashscope.aliyuncs.com/compatible-mode/v1` |

> `VITE_*` 会打进前端包，不要把真实密钥提交进仓库。页面设置里的配置保存在浏览器 `localStorage`。

---

## 使用示例

试试在聊天框输入：

- 「画一个简单的流程图：开始 → 处理 → 结束」
- 「画一个前后端分离的架构图」
- 「把选中的框改成红色」
- 「在现有图下方补充一个数据库节点」

---

## 技术栈

React 19 · TypeScript · Vite 7 · Tailwind CSS v4 · [Excalidraw](https://excalidraw.com/) · Radix UI · Bun · Vitest · GitHub Actions

<details>
<summary><strong>目录结构</strong></summary>

```
ai-excalidraw/
├── src/
│   ├── components/
│   │   ├── excalidraw/          # 编辑器、画布、聊天、流式解析
│   │   │   ├── index.tsx        # 桌面 / 移动布局入口
│   │   │   ├── wrapper.tsx      # Excalidraw 封装与本地画布存储
│   │   │   ├── chat-panel.tsx   # 桌面端聊天
│   │   │   ├── mobile-input.tsx # 移动端输入与会话
│   │   │   ├── use-ai-draw.ts   # 共享发送 / 工具调用逻辑
│   │   │   ├── element-parser.ts
│   │   │   └── use-chat-history.ts
│   │   ├── settings-dialog.tsx  # API、语言、备份
│   │   └── ui/
│   ├── lib/
│   │   ├── ai.ts                # SSE、工具调用、多轮历史
│   │   ├── prompt.ts            # 系统提示词 / 输出协议
│   │   ├── message-content.ts   # 消息清洗
│   │   ├── storage.ts           # 防抖写入与配额错误
│   │   ├── backup.ts            # 导入导出
│   │   └── i18n.ts
│   ├── App.tsx
│   └── main.tsx
├── .github/workflows/ci.yml
├── CONTRIBUTING.md
└── package.json
```

</details>

---

## 部署

构建后为静态站点，可部署到任意静态托管（GitHub Pages、Cloudflare Pages、Vercel、Netlify 等）：

```bash
bun run build
# 上传 dist/
```

自托管时如需预置 API，可设置构建期环境变量 `VITE_AI_API_KEY` / `VITE_AI_BASE_URL` / `VITE_AI_MODEL`（注意密钥会暴露给浏览器）。

---

## 贡献

见 [CONTRIBUTING.md](./CONTRIBUTING.md)。提交 PR 前请确保：

```bash
bun run lint && bun run test && bun run build
```

## 开源协议

[MIT](./LICENSE)

## 支持作者

如果这个项目对你有帮助，欢迎请作者喝瓶水：

<img src="./assets/donate.jpg" alt="赞赏码" width="200" />
