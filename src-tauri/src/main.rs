#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agent_project;
mod agent_runs;
mod bridge;
mod integrations;
mod lifecycle;
mod local_chat;
mod mcp;
mod mcp_stdio;
mod storage;
mod workspace;

use std::sync::atomic::Ordering;
use tauri::{Emitter, Manager};

fn main() -> Result<(), tauri::Error> {
    if std::env::args().any(|arg| arg == "--mcp-stdio") {
        if let Err(error) = mcp_stdio::run() {
            eprintln!("MCP connector: {error}");
            std::process::exit(1);
        }
        return Ok(());
    }
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            lifecycle::open_files(
                app,
                args.into_iter()
                    .filter(|arg| arg.ends_with(".excalidraw"))
                    .collect(),
            );
            if let Some(window) = app.get_webview_window("main") {
                if let Err(error) = window.show().and_then(|()| window.set_focus()) {
                    eprintln!("Cannot focus the canvas window: {error}");
                }
            }
        }))
        .plugin(tauri_plugin_http::init())
        .manage(workspace::WorkspaceState::default())
        .manage(lifecycle::Lifecycle::default())
        .manage(local_chat::LocalChatState::default())
        .setup(|app| {
            let data = app.path().app_data_dir()?;
            if let Err(error) = agent_runs::reconcile_scope(&data, None) {
                eprintln!("Agent run recovery: {error}");
            }
            match workspace::load_registry(&data) {
                Ok(registry) => {
                    for project in registry.projects {
                        if let Err(error) = agent_runs::reconcile_scope(&data, Some(project.id)) {
                            eprintln!("Project Agent run recovery: {error}");
                        }
                    }
                }
                Err(error) => eprintln!("Cannot load Agent recovery registry: {error}"),
            }
            lifecycle::open_files(
                app.handle(),
                std::env::args()
                    .skip(1)
                    .filter(|arg| arg.ends_with(".excalidraw"))
                    .collect(),
            );
            bridge::start(app.handle())
        })
        .invoke_handler(tauri::generate_handler![
            bridge::complete_canvas_command,
            bridge::canvas_bridge_ready,
            storage::save_canvas_snapshot,
            storage::remove_canvas_snapshot,
            storage::load_canvas_snapshots,
            workspace::workspace_command,
            lifecycle::take_launch_files,
            integrations::integration_status,
            integrations::integration_install,
            integrations::integration_claim_prompt,
            integrations::integration_guide,
            local_chat::local_chat_tools,
            local_chat::local_chat_start,
            local_chat::local_chat_cancel,
            agent_runs::agent_run_command,
            agent_project::agent_project_command,
            lifecycle::workspace_exit
        ])
        .build(tauri::generate_context!())?;
    app.run(|handle, event| match event {
        tauri::RunEvent::ExitRequested { api, .. } => {
            if !handle
                .state::<lifecycle::Lifecycle>()
                .exiting
                .load(Ordering::Acquire)
            {
                api.prevent_exit();
                if let Err(error) = handle.emit_to("main", "flush-before-exit", ()) {
                    eprintln!("Cannot request save before exit: {error}");
                }
            }
        }
        tauri::RunEvent::WindowEvent {
            event: tauri::WindowEvent::CloseRequested { api, .. },
            ..
        } => {
            api.prevent_close();
            if let Err(error) = handle.emit_to("main", "flush-before-exit", ()) {
                eprintln!("Cannot request save before close: {error}");
            }
        }
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Opened { urls } => lifecycle::open_files(
            handle,
            urls.iter()
                .filter_map(|url| url.to_file_path().ok())
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
        ),
        tauri::RunEvent::Exit => {
            handle.state::<local_chat::LocalChatState>().cancel_all();
            bridge::cleanup(handle);
        }
        _ => {}
    });
    Ok(())
}
