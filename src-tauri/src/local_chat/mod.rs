mod cli;
mod codex_server;
mod error;
mod events;
mod process;
#[cfg(all(test, unix))]
mod process_tests;
mod project;
#[cfg(test)]
mod project_tests;
mod state;
mod stream;
#[cfg(test)]
mod tests;

use crate::integrations::{self, ToolId};
use error::ChatError;
pub use events::LocalChatEvent;
use serde::Serialize;
pub use state::LocalChatState;
use tauri::{ipc::Channel, AppHandle, Manager, State};

#[derive(Serialize)]
pub struct LocalChatTool {
    id: ToolId,
    name: &'static str,
    available: bool,
    note: Option<&'static str>,
    capabilities: codex_server::capabilities::Capabilities,
    #[serde(rename = "prefersAppServer")]
    prefers_app_server: bool,
}

#[tauri::command]
pub async fn local_chat_tools(app: AppHandle) -> Result<Vec<LocalChatTool>, String> {
    let context = integrations::context(&app)?;
    let mut entries = Vec::new();
    for id in ToolId::ALL {
        let binary = context.cli_binary(id);
        let native = if matches!(id, ToolId::Codex) {
            if let Some(binary) = &binary {
                codex_server::capabilities::detect(binary).await
            } else {
                false
            }
        } else {
            false
        };
        entries.push(LocalChatTool {
            id,
            name: id.name(),
            available: binary.is_some(),
            note: Some(if native { "使用工具自身登录与模型配置。运行中补充暂不支持；当前轮完成或停止后，使用同一 conversationId 继续新一轮。" } else { "使用工具自身登录与模型配置；能力探测不验证登录状态。" }),
            capabilities: codex_server::capabilities::Capabilities::for_tool(id, native),
            prefers_app_server: native,
        });
    }
    Ok(entries)
}

// Flat arguments are required by the agreed Tauri IPC contract.
#[tauri::command]
#[expect(
    clippy::too_many_arguments,
    reason = "Flat IPC contract includes injected app/state/channel and optional canvas context"
)]
pub async fn local_chat_start(
    app: AppHandle,
    state: State<'_, LocalChatState>,
    request_id: String,
    tool: ToolId,
    prompt: String,
    on_event: Channel<LocalChatEvent>,
    project_id: Option<String>,
    canvas_id: Option<String>,
    conversation_id: Option<String>,
) -> Result<(), String> {
    let mut lease = state
        .register(request_id, &prompt)
        .map_err(|e| e.to_string())?;
    let context = integrations::context(&app)?;
    state::validate_canvas(project_id.as_deref(), canvas_id.as_deref())
        .map_err(|e| e.to_string())?;
    let data = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let root = project_id
        .as_deref()
        .map(|id| crate::workspace::project_root(&data, id))
        .transpose()?;
    if canvas_id.is_some() && root.is_none() {
        return Err(ChatError::CanvasContext.to_string());
    }
    let native = if matches!(tool, ToolId::Codex) {
        if let Some(binary) = context.cli_binary(tool) {
            codex_server::capabilities::detect(&binary).await
        } else {
            false
        }
    } else {
        false
    };
    let mut store = if matches!(tool, ToolId::Codex) {
        Some(
            codex_server::Store::open(&data, conversation_id.clone(), root.clone(), canvas_id)
                .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };
    let invocation = tauri::async_runtime::spawn_blocking(move || {
        if native {
            cli::prepare_backend(&context, tool, root, true)
        } else {
            cli::prepare(&context, tool, root)
        }
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    let mut sink: stream::Sink =
        Box::new(move |event| on_event.send(event).map_err(|_| ChatError::Channel));
    if native {
        codex_server::execute(
            invocation,
            &prompt,
            &mut sink,
            &mut lease.control,
            store.as_mut().ok_or("Missing session store")?,
        )
        .await
        .map_err(|e| e.to_string())?;
    } else {
        process::execute_compatibility(
            invocation,
            process::ChatInput {
                prompt: &prompt,
                events: sink,
            },
            &mut lease.control,
        )
        .await
        .map_err(|e| e.to_string())?;
    }
    if *lease.control.cancel.borrow() {
        return Err(ChatError::Cancelled.to_string());
    }
    Ok(())
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri extracts command arguments by value"
)]
pub fn local_chat_cancel(
    state: State<'_, LocalChatState>,
    request_id: String,
) -> Result<(), String> {
    state.cancel(&request_id).map_err(|e| e.to_string())
}
