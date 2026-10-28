use super::{config, files, model::Context, PAYLOAD, SKILL_NAME};
use serde::Serialize;
use serde_json::json;
use std::path::Path;

#[derive(Serialize)]
pub struct Guide {
    pub text: String,
}

pub(super) fn export(context: &Context, root: Option<&Path>) -> Result<Guide, String> {
    let source = match root {
        Some(root) => {
            let (config, skill) = super::project::paths(context, super::ToolId::Codex, Some(root));
            files::safe_path(&config)?;
            skill
        }
        None => context
            .bridge
            .parent()
            .ok_or("缺少应用数据目录")?
            .join("integrations")
            .join(SKILL_NAME),
    };
    files::skill_status(&source)?;
    let metadata = match root {
        Some(root) => json!({
            "projectRoot": root,
            "mcpConfigPath": root.join(".agents/mcp.json"),
            "skillPath": source,
            "skillSourcePath": source
        }),
        None => json!({"skillSourcePath": source}),
    };
    let metadata = serde_json::to_string_pretty(&metadata).map_err(|e| e.to_string())?;
    let connector = serde_json::to_string_pretty(&json!({"mcpServers": {"ai-excalidraw": {
        "type": "stdio", "command": context.executable, "args": ["--mcp-stdio"],
        "env": {"AI_EXCALIDRAW_BRIDGE_FILE": context.bridge}
    }}}))
    .map_err(|e| e.to_string())?;
    let scope = match root {
        Some(_) => "当前画布项目已选定。共享安装位置是下面 projectRoot 内的 .agents/mcp.json（JSON mcpServers）与 .agents/skills/ai-excalidraw-desktop。Skill 已导出到该项目；连接配置仅合并到 mcpConfigPath，不额外创建项目 .claude、.cursor 等原生配置，也不写入全局用户目录。应用内本地聊天的运行时适配器会读取共享项目配置，转换为各工具支持的原生配置或启动参数；需要时通过临时插件提供 Skill 及其工具原生命名空间，并保留工具自身登录和认证。独立启动的外部 CLI 不会经过这些内置适配器，不能假定它自动发现 .agents/mcp.json 或以同一名称加载 Skill；必须按该工具和版本支持的原生配置、插件或启动参数接入。请说明外部启动所需的具体适配方式；本次指引的写入范围仍仅为 .agents，其他位置的写入须另有明确授权。",
        None => "当前没有选择画布项目，使用工具的用户级安装位置。若需要项目级安装，请先选择画布项目，再重新获取指引。",
    };
    let codex = if root.is_none() {
        // Render from an empty document; guide generation never reads client configuration.
        let toml = config::native_toml(context)?;
        format!("若当前工具是 Codex，可使用以下 TOML 条目；请先确认 CODEX_HOME 或其实际用户配置路径：\n```toml\n{toml}```\n")
    } else {
        String::new()
    };
    let skill_instructions = match root {
        Some(_) => "项目 Skill 的七个文件（SKILL.md、references/elements.md、references/local-setup.md、scripts/canvas.mjs、agents/openai.yaml、assets/LICENSE-ai-excalidraw.txt、VERSION）已位于 skillPath，保持相对路径，直接从该目录读取。不要另行复制到全局目录；保留其他自定义文件，拒绝不安全的符号链接路径。",
        None => "请将源目录中的七个文件（SKILL.md、references/elements.md、references/local-setup.md、scripts/canvas.mjs、agents/openai.yaml、assets/LICENSE-ai-excalidraw.txt、VERSION）复制到该工具支持的 ai-excalidraw-desktop Skill 目录，保持相对路径。覆盖前备份原文件，保留其他自定义文件，拒绝不安全的符号链接路径。",
    };
    let text = format!(
        r"请为我当前使用的 AI 工具接入本机 AI Excalidraw 桌面应用的原生 MCP 和 Skill。

桌面应用启动时自动启动自己的 MCP 服务，服务运行不依赖任何 AI 开发工具。下面的配置仅用于客户端连接。请勿自动启动或重启桌面应用；若应用未运行，请提示我手动打开。连接器是现有桌面程序的 --mcp-stdio 模式，无需安装 Node.js、npm 包或其他运行时。

{scope}

请先识别当前 AI 工具及版本，查阅其官方文档并核实是否支持本地 stdio MCP 与 Skill，确定其真实配置路径和 Skill 目录，不要猜测位置。不支持的部分请明确说明，不要修改不兼容的配置。

Skill 源目录（绝对路径，JSON 转义）：
```json
{metadata}
```
{skill_instructions}

通用 JSON stdio 配置示例（按当前工具的官方结构合并）：
```json
{connector}
```

{codex}

修改前先备份配置，解析失败时停止并报告，禁止重置整个文件。仅合并 ai-excalidraw 这一 MCP 条目的传输字段：command、args 和 env.AI_EXCALIDRAW_BRIDGE_FILE；JSON 客户端需要时设 type 为 stdio，移除该条目中冲突的旧 HTTP URL 和 headers。保留其他服务器、无关环境变量及已有审批、enabled/disabled、工具权限等策略。采用原子写入，失败时恢复备份。

桥接文件路径用于绑定当前桌面实例。请勿读取、输出或复制其中的 bearer token，也不要把 token 写入客户端配置或对话。本指引仅提供路径，不包含凭据。

完成后报告修改路径和备份位置，提示我按工具要求重新加载 MCP 与 Skill；在桌面应用已运行时执行只读连接验证，不创建或改动画布。
"
    );
    let mut transaction = files::Transaction::default();
    for (relative, bytes) in PAYLOAD {
        if let Err(error) = transaction.write(&source.join(relative), bytes) {
            return match transaction.rollback() {
                Ok(()) => Err(error),
                Err(rollback) => Err(format!("{error}；回滚失败：{rollback}")),
            };
        }
    }
    transaction.commit();
    Ok(Guide { text })
}

#[cfg(test)]
#[path = "guide_tests.rs"]
mod tests;
