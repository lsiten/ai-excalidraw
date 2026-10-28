use crate::integrations::ToolId;

#[derive(Debug, thiserror::Error)]
pub(super) enum ChatError {
    #[error("请求编号须为 1–128 位字母、数字、横线或下划线；提示词不能为空且不能超过 2 MiB。")]
    InvalidRequest,
    #[error("画布编号无效或与当前项目不匹配，请重新选择画布。")]
    CanvasContext,
    #[error("相同请求编号的本地对话仍在运行。")]
    Duplicate,
    #[error("本地对话请求过多，请等待或停止现有请求后重试。")]
    Busy,
    #[error("本地对话状态不可用，请重启应用。")]
    State,
    #[error("未找到 {0} 命令行工具，请安装并在终端登录后重试。")]
    Missing(&'static str),
    #[error("本地对话已停止。")]
    Cancelled,
    #[error("本地对话超过五分钟时限，已停止。")]
    Timeout,
    #[error("本地工具输出超过限制（事件流 4 MiB、诊断信息 64 KiB），已停止。")]
    OutputLimit,
    #[error("项目 .agents/mcp.json 配置无效或超过 1 MiB，请检查 mcpServers 对象。")]
    ProjectConfig,
    #[error("本地工具返回了无效事件流或缺少完成事件，请更新工具并在终端检查配置。")]
    Protocol,
    #[error("无法{operation}：{source}")]
    Io {
        operation: &'static str,
        source: std::io::Error,
    },
    #[error("{0}")]
    Cli(String),
    #[error("对话事件通道已关闭。")]
    Channel,
}

pub(super) fn io(operation: &'static str) -> impl FnOnce(std::io::Error) -> ChatError {
    move |source| ChatError::Io { operation, source }
}

pub(super) enum FailureOrigin {
    Command,
    Agent,
}

pub(super) fn cli_failure(tool: ToolId, diagnostics: &str, origin: FailureOrigin) -> ChatError {
    let lower = diagnostics.to_ascii_lowercase();
    let authentication = [
        "unauthorized",
        "authentication",
        "not logged",
        "login",
        "api key",
        "401",
        "403",
    ]
    .iter()
    .any(|s| lower.contains(s));
    let argument_error = match origin {
        FailureOrigin::Command => [
            "unknown option",
            "unexpected argument",
            "unrecognized option",
            "unrecognized argument",
            "invalid option",
        ]
        .iter()
        .any(|s| lower.contains(s)),
        FailureOrigin::Agent => false,
    };
    let reason = if authentication {
        "登录验证失败。"
    } else if ["429", "rate limit", "quota", "credit", "billing"]
        .iter()
        .any(|s| lower.contains(s))
    {
        "服务商报告了用量或账单限制。"
    } else if argument_error {
        "此版本不支持所需的非交互参数，请更新命令行工具。"
    } else if ["proxy", "connect", "network", "fetch failed"]
        .iter()
        .any(|s| lower.contains(s))
    {
        "请检查工具自身的网络和代理设置。"
    } else if lower.contains("permission") || lower.contains("denied") {
        "工具权限被拒绝，请在该工具中配置所需权限后重试。"
    } else {
        "命令行工具未返回有效结果，请在终端检查模型、服务商和项目配置。"
    };
    let login = if authentication {
        match tool {
            ToolId::Codex => "可在终端运行 codex login 登录。",
            ToolId::Claude => "可在终端运行 claude auth login 登录。",
            ToolId::Cursor => "可在终端运行 cursor-agent login（或 agent login）登录。",
            ToolId::Pi => "可在终端打开 pi 并使用 /login 配置服务商。",
        }
    } else {
        ""
    };
    ChatError::Cli(format!(
        "{}：{reason}{login}使用工具自身的模型和登录配置。",
        tool.name()
    ))
}
