#![expect(
    clippy::unwrap_used,
    reason = "Tests fail immediately on fixture errors"
)]
use super::super::{
    cli::Invocation, error::ChatError, events::LocalChatEvent, process::ProcessControl,
    stream::Sink,
};
use super::{
    capabilities, execute,
    session::{session, Progress},
    transport::send,
    watchdog::Limits,
    Store,
};
use serde_json::{json, Value};
use std::{path::Path, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    time::Instant,
};
#[tokio::test]
async fn startup_resume_failure_warning_and_no_replay_over_wire() {
    for resumed in [false, true] {
        for status in ["completed", "failed"] {
            let data = tempfile::tempdir().unwrap();
            let mut store =
                Store::open(data.path(), Some("conversation".into()), None, None).unwrap();
            if resumed {
                store.save("thread").unwrap();
            }
            let (client, peer) = tokio::io::duplex(16384);
            let (read, mut write) = tokio::io::split(client);
            let peer_run = async {
                let (read, mut write) = tokio::io::split(peer);
                let mut lines = BufReader::new(read).lines();
                for (method, response) in [
                    ("initialize", json!({"id":1,"result":{}})),
                    ("initialized", Value::Null),
                    (
                        if resumed {
                            "thread/resume"
                        } else {
                            "thread/start"
                        },
                        json!({"id":2,"result":{"thread":{"id":"thread"}}}),
                    ),
                    (
                        "turn/start",
                        json!({"id":3,"result":{"turn":{"id":"turn"}}}),
                    ),
                ] {
                    let request: Value =
                        serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
                    assert_eq!(request["method"], method);
                    if method.starts_with("thread/") {
                        assert_eq!(request["params"]["sandbox"], "read-only");
                    }
                    if !response.is_null() {
                        send(&mut write, response).await.unwrap();
                    }
                }
                send(&mut write,json!({"method":"item/reasoning/summaryTextDelta","params":{"delta":"summary"}})).await.unwrap();
                send(&mut write,json!({"method":"thread/tokenUsage/updated","params":{"tokenUsage":{"total":{"inputTokens":12,"outputTokens":3,"cachedInputTokens":4,"totalTokens":15}}}})).await.unwrap();
                send(
                    &mut write,
                    json!({"method":"warning","params":{"message":"recoverable"}}),
                )
                .await
                .unwrap();
                send(&mut write,json!({"method":"turn/plan/updated","params":{"plan":[{"step":"inspect","status":"completed"}]}})).await.unwrap();
                send(&mut write,json!({"method":"turn/completed","params":{"turn":{"id":"turn","status":status}}})).await.unwrap();
            };
            let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
            let output = events.clone();
            let mut sink: Sink = Box::new(move |event| {
                output.lock().unwrap().push(event);
                Ok(())
            });
            let mut progress = Progress {
                limits: Limits::default(),
                submitted: false,
                thread: None,
                turn: None,
            };
            let (result, ()) = tokio::join!(
                session(
                    read,
                    &mut write,
                    "hello",
                    &mut sink,
                    &mut store,
                    data.path(),
                    &mut progress
                ),
                peer_run
            );
            assert_eq!(result.is_ok(), status == "completed");
            println!(
                "wire resumed={resumed} terminal={status} ok={}",
                result.is_ok()
            );
            assert!(progress.submitted);
            assert!(events
                .lock()
                .unwrap()
                .iter()
                .any(|e| matches!(e,LocalChatEvent::Session{resumed:r,..} if *r==resumed)));
            assert!(events
                .lock()
                .unwrap()
                .iter()
                .any(|e| matches!(e, LocalChatEvent::Status { .. })));
            assert!(events
                .lock()
                .unwrap()
                .iter()
                .any(|e| matches!(e,LocalChatEvent::Thinking{text} if text=="summary")));
            assert!(events.lock().unwrap().iter().any(|e| matches!(
                e,
                LocalChatEvent::Usage {
                    input_tokens: 12,
                    output_tokens: 3,
                    cached_input_tokens: 4,
                    total_tokens: 15
                }
            )));
            assert!(
                capabilities::Capabilities::for_tool(crate::integrations::ToolId::Codex, true)
                    .supports_summary
            );
            drop(store);
            assert_eq!(
                Store::open(data.path(), Some("conversation".into()), None, None)
                    .unwrap()
                    .thread
                    .as_deref(),
                Some("thread")
            );
        }
    }
}
#[test]
fn store_rejects_scope_mismatch_and_concurrent_turns() {
    let data = tempfile::tempdir().unwrap();
    let mut store = Store::open(
        data.path(),
        Some("same".into()),
        None,
        Some("canvas".into()),
    )
    .unwrap();
    store.save("thread").unwrap();
    assert!(Store::open(
        data.path(),
        Some("same".into()),
        None,
        Some("canvas".into())
    )
    .is_err());
    drop(store);
    assert!(Store::open(data.path(), Some("same".into()), None, Some("other".into())).is_err());
    assert!(Store::open(data.path(), Some("../escape".into()), None, None).is_err());
}

#[tokio::test]
#[ignore = "explicit installed Codex readonly smoke; requires existing login"]
async fn installed_codex_readonly_start_and_resume() {
    let binary = std::env::var_os("CODEX_SMOKE_BINARY").unwrap();
    assert!(capabilities::detect(Path::new(&binary)).await);
    let data = tempfile::tempdir().unwrap();
    for round in 0..2 {
        let binary_path = std::path::PathBuf::from(&binary);
        let home = std::path::PathBuf::from(std::env::var_os("HOME").unwrap());
        let context = crate::integrations::Context {
            codex: std::env::var_os("CODEX_HOME")
                .map_or_else(|| home.join(".codex"), std::path::PathBuf::from),
            home,
            executable: std::env::current_exe().unwrap(),
            bridge: data.path().join("unused-bridge"),
            search: vec![binary_path.parent().unwrap().to_path_buf()],
            applications: vec![],
        };
        let invocation = super::super::cli::prepare_backend(
            &context,
            crate::integrations::ToolId::Codex,
            Some(data.path().into()),
            true,
        )
        .unwrap();
        assert_eq!(
            invocation.command.as_std().get_program(),
            binary_path.as_os_str()
        );
        let mut store = Store::open(
            data.path(),
            Some("smoke".into()),
            Some(data.path().into()),
            None,
        )
        .unwrap();
        assert_eq!(store.thread.is_some(), round == 1);
        let (_tx, cancel) = tokio::sync::watch::channel(false);
        let mut control = ProcessControl {
            cancel,
            process: std::sync::Arc::default(),
            duration: Duration::from_secs(90),
        };
        let captured = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let output = captured.clone();
        let mut sink: Sink = Box::new(move |event| {
            assert!(!matches!(event, LocalChatEvent::Tool { .. }));
            if let LocalChatEvent::Chunk { text } = &event {
                output.lock().unwrap().push_str(text);
            }
            println!("{}", serde_json::to_string(&event).unwrap());
            Ok(())
        });
        let prompt = if round == 0 {
            "Do not use tools or change files. Remember the word cobalt. Reply only ACK."
        } else {
            "Do not use tools or change files. What word did I ask you to remember? Reply only that word."
        };
        execute(invocation, prompt, &mut sink, &mut control, &mut store)
            .await
            .unwrap();
        assert_eq!(
            captured.lock().unwrap().trim().to_lowercase(),
            if round == 0 { "ack" } else { "cobalt" }
        );
        println!("round={round} persisted={}", store.thread.is_some());
    }
    let temporary_path = data.path().to_owned();
    data.close().unwrap();
    assert!(!temporary_path.exists());
    println!("temporary_project_and_session_mapping_removed=true");
}

#[cfg(unix)]
#[tokio::test]
async fn cancellation_interrupts_and_reaps_real_fake_process() {
    let data = tempfile::tempdir().unwrap();
    let script = data.path().join("peer.py");
    let trace = data.path().join("trace");
    std::fs::write(&script,r"
import sys,json
trace=open(sys.argv[1],'w',buffering=1)
for line in sys.stdin:
 r=json.loads(line); m=r['method'];trace.write(m+'\n')
 if m=='initialized':continue
 if m=='turn/interrupt':break
 result={}
 if m=='thread/start':result={'thread':{'id':'thread'}}
 if m=='turn/start':result={'turn':{'id':'turn'}}
 print(json.dumps({'id':r['id'],'result':result}),flush=True)
 if m=='turn/start':print(json.dumps({'method':'item/agentMessage/delta','params':{'delta':'ready'}}),flush=True)
").unwrap();
    let mut command = tokio::process::Command::new("/usr/bin/python3");
    command
        .arg(&script)
        .arg(&trace)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let invocation = Invocation {
        command,
        directory: tempfile::tempdir().unwrap(),
        tool: crate::integrations::ToolId::Codex,
        project_root: None,
    };
    let (tx, cancel) = tokio::sync::watch::channel(false);
    let mut control = ProcessControl {
        cancel,
        process: std::sync::Arc::default(),
        duration: Duration::from_secs(5),
    };
    let mut store = Store::open(data.path(), None, None, None).unwrap();
    let mut sink: Sink = Box::new(move |event| {
        if matches!(event, LocalChatEvent::Chunk { .. }) {
            tx.send_replace(true);
        }
        Ok(())
    });
    let result = execute(invocation, "test", &mut sink, &mut control, &mut store).await;
    assert!(matches!(result, Err(ChatError::Cancelled)));
    let trace = std::fs::read_to_string(trace).unwrap();
    assert_eq!(
        trace.lines().filter(|line| *line == "turn/start").count(),
        1
    );
    assert!(trace.contains("turn/interrupt"));
    println!("cancelled process transcript:\n{trace}");
}

#[tokio::test]
async fn startup_protocol_error_does_not_submit_thread() {
    let data = tempfile::tempdir().unwrap();
    let mut store = Store::open(data.path(), None, None, None).unwrap();
    let (client, peer) = tokio::io::duplex(4096);
    let (read, mut write) = tokio::io::split(client);
    let peer_run = async {
        let (read, mut write) = tokio::io::split(peer);
        let mut lines = BufReader::new(read).lines();
        assert!(lines.next_line().await.unwrap().is_some());
        send(
            &mut write,
            json!({"id":1,"error":{"code":-32600,"message":"bad initialize"}}),
        )
        .await
        .unwrap();
    };
    let mut sink: Sink = Box::new(|_| Ok(()));
    let mut progress = Progress {
        limits: Limits::default(),
        submitted: false,
        thread: None,
        turn: None,
    };
    let (result, ()) = tokio::join!(
        session(
            read,
            &mut write,
            "test",
            &mut sink,
            &mut store,
            data.path(),
            &mut progress
        ),
        peer_run
    );
    assert!(result.is_err());
    assert!(!progress.submitted);
    assert!(store.thread.is_none());
}

#[cfg(unix)]
#[tokio::test]
async fn total_watchdog_reaps_silent_process_and_probe_is_readonly() {
    let data = tempfile::tempdir().unwrap();
    let script = data.path().join("silent.py");
    std::fs::write(&script, "import time\ntime.sleep(30)\n").unwrap();
    let mut command = tokio::process::Command::new("/usr/bin/python3");
    command
        .arg(script)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let invocation = Invocation {
        command,
        directory: tempfile::tempdir().unwrap(),
        tool: crate::integrations::ToolId::Codex,
        project_root: None,
    };
    let (_tx, cancel) = tokio::sync::watch::channel(false);
    let mut control = ProcessControl {
        cancel,
        process: std::sync::Arc::default(),
        duration: Duration::from_millis(20),
    };
    let mut store = Store::open(data.path(), None, None, None).unwrap();
    let mut sink: Sink = Box::new(|_| Ok(()));
    let started = Instant::now();
    assert!(matches!(
        execute(invocation, "test", &mut sink, &mut control, &mut store).await,
        Err(ChatError::Timeout)
    ));
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(store.thread.is_none());
    assert!(!capabilities::detect(Path::new("/nonexistent-codex-test-binary")).await);
    assert!(!capabilities::detect(Path::new("/usr/bin/false")).await);
}

#[tokio::test]
async fn interactive_request_is_rejected_without_approval_claim() {
    let data = tempfile::tempdir().unwrap();
    let mut store = Store::open(data.path(), None, None, None).unwrap();
    let (client, peer) = tokio::io::duplex(4096);
    let (read, mut write) = tokio::io::split(client);
    let peer_run = async {
        let (read, mut write) = tokio::io::split(peer);
        let mut lines = BufReader::new(read).lines();
        assert!(lines.next_line().await.unwrap().is_some());
        send(
            &mut write,
            json!({"id":"approval","method":"item/commandExecution/requestApproval","params":{}}),
        )
        .await
        .unwrap();
        let response: Value =
            serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert_eq!(response["id"], "approval");
        assert_eq!(response["error"]["code"], -32601);
    };
    let mut sink: Sink = Box::new(|event| {
        if let LocalChatEvent::Input { supported, .. } = event {
            assert!(!supported);
        }
        Ok(())
    });
    let mut progress = Progress {
        limits: Limits::default(),
        submitted: false,
        thread: None,
        turn: None,
    };
    let (result, ()) = tokio::join!(
        session(
            read,
            &mut write,
            "test",
            &mut sink,
            &mut store,
            data.path(),
            &mut progress
        ),
        peer_run
    );
    assert!(result.is_err());
    assert!(
        !capabilities::Capabilities::for_tool(crate::integrations::ToolId::Codex, true)
            .supports_approval
    );
}

#[tokio::test]
async fn handshake_startup_idle_and_tool_watchdogs_bound_stalled_wire() {
    for stall in ["initialize", "thread/start", "turn/start", "idle", "tool"] {
        let data = tempfile::tempdir().unwrap();
        let mut store = Store::open(data.path(), None, None, None).unwrap();
        let (client, peer) = tokio::io::duplex(16384);
        let (read, mut write) = tokio::io::split(client);
        let peer_run = async {
            let (read, mut write) = tokio::io::split(peer);
            let mut lines = BufReader::new(read).lines();
            loop {
                let request: Value =
                    serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
                let method = request["method"].as_str().unwrap();
                if method == stall {
                    std::future::pending::<()>().await;
                }
                let result = match method {
                    "initialized" => continue,
                    "thread/start" => json!({"thread":{"id":"thread"}}),
                    "turn/start" => json!({"turn":{"id":"turn"}}),
                    _ => json!({}),
                };
                send(&mut write, json!({"id":request["id"],"result":result}))
                    .await
                    .unwrap();
                if method == "turn/start" {
                    if stall == "tool" {
                        send(&mut write,json!({"method":"item/started","params":{"item":{"id":"tool","type":"commandExecution"}}})).await.unwrap();
                    } else {
                        send(
                            &mut write,
                            json!({"method":"item/agentMessage/delta","params":{"delta":"hello"}}),
                        )
                        .await
                        .unwrap();
                    }
                    std::future::pending::<()>().await;
                }
            }
        };
        let mut progress = Progress {
            limits: Limits {
                handshake: Duration::from_millis(20),
                startup: Duration::from_millis(20),
                idle: Duration::from_millis(40),
                tool: Duration::from_millis(10),
                human: Duration::from_millis(100),
            },
            submitted: false,
            thread: None,
            turn: None,
        };
        let mut sink: Sink = Box::new(|_| Ok(()));
        let result = tokio::select! {result=session(read,&mut write,"test",&mut sink,&mut store,data.path(),&mut progress)=>result, ()=peer_run=>panic!("peer exited")};
        assert!(matches!(result, Err(ChatError::Timeout)), "stall={stall}");
        println!("watchdog stall={stall} timed_out=true");
    }
}

#[cfg(unix)]
#[test]
fn native_preparation_preserves_project_config_and_readonly_policy() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let binary = root.path().join("codex");
    std::fs::write(&binary, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::create_dir(root.path().join(".agents")).unwrap();
    let config = root.path().join(".agents/mcp.json");
    let original = r#"{"mcpServers":{"canvas":{"command":"project-tool"}}}"#;
    std::fs::write(&config, original).unwrap();
    let context = crate::integrations::Context {
        home: root.path().into(),
        codex: root.path().join("home"),
        executable: binary,
        bridge: root.path().join("bridge"),
        search: vec![root.path().into()],
        applications: vec![],
    };
    let invocation = super::super::cli::prepare_backend(
        &context,
        crate::integrations::ToolId::Codex,
        Some(root.path().into()),
        true,
    )
    .unwrap();
    let args: Vec<_> = invocation
        .command
        .as_std()
        .get_args()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    assert_eq!(args[0], "app-server");
    assert!(args.iter().any(|arg| arg == "sandbox_mode=\"read-only\""));
    assert!(args
        .iter()
        .any(|arg| arg.starts_with("mcp_servers.\"canvas\"=")));
    assert!(!args
        .iter()
        .any(|arg| arg.contains("bypass") || arg.contains("ephemeral")));
    assert_eq!(std::fs::read_to_string(config).unwrap(), original);
    assert_eq!(
        invocation.command.as_std().get_current_dir(),
        Some(root.path())
    );
}

#[test]
fn persisted_mapping_rejects_backend_and_project_switches() {
    let data = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let mut store = Store::open(
        data.path(),
        Some("bound".into()),
        Some(project.path().into()),
        None,
    )
    .unwrap();
    store.save("thread").unwrap();
    drop(store);
    assert!(Store::open(data.path(), Some("bound".into()), None, None).is_err());
    let path = data.path().join("local-chat-sessions/bound.json");
    let mut mapping: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    mapping["backend"] = json!("claude-cli");
    std::fs::write(&path, mapping.to_string()).unwrap();
    assert!(Store::open(
        data.path(),
        Some("bound".into()),
        Some(project.path().into()),
        None
    )
    .is_err());
}

#[cfg(unix)]
#[tokio::test]
async fn legacy_codex_accepts_platform_conversation_with_complete_history() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let binary = root.path().join("codex");
    std::fs::write(&binary,r#"#!/usr/bin/python3
import sys,json,pathlib
if '--help' in sys.argv:
 print('Usage: codex exec');sys.exit(0)
assert sys.argv[1]=='exec'
payload=json.load(sys.stdin)
pathlib.Path('received.json').write_text(json.dumps(payload))
print(json.dumps({'type':'item.completed','item':{'id':'reply','type':'agent_message','text':'ok'}}),flush=True)
print(json.dumps({'type':'turn.completed'}),flush=True)
"#).unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(!capabilities::detect(&binary).await);
    let mut store = Store::open(
        root.path(),
        Some("platform-conversation".into()),
        Some(root.path().into()),
        None,
    )
    .unwrap();
    store.save("older-native-thread").unwrap();
    let history = json!({"context":{"conversationId":"platform-conversation"},"messages":[{"role":"user","content":"a"},{"role":"assistant","content":"b"},{"role":"user","content":"c"}]});
    let invocation = Invocation {
        command: super::super::cli::command(
            crate::integrations::ToolId::Codex,
            &binary,
            root.path(),
        ),
        directory: tempfile::tempdir().unwrap(),
        tool: crate::integrations::ToolId::Codex,
        project_root: Some(root.path().into()),
    };
    let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let output = events.clone();
    let sink: Sink = Box::new(move |event| {
        output.lock().unwrap().push(event);
        Ok(())
    });
    let (_tx, cancel) = tokio::sync::watch::channel(false);
    let mut control = ProcessControl {
        cancel,
        process: std::sync::Arc::default(),
        duration: Duration::from_secs(5),
    };
    super::super::process::execute_compatibility(
        invocation,
        super::super::process::ChatInput {
            prompt: &history.to_string(),
            events: sink,
        },
        &mut control,
    )
    .await
    .unwrap();
    let received: Value =
        serde_json::from_slice(&std::fs::read(root.path().join("received.json")).unwrap()).unwrap();
    assert_eq!(received, history);
    assert!(matches!(
        events.lock().unwrap().first(),
        Some(LocalChatEvent::Status { .. })
    ));
    assert!(!events
        .lock()
        .unwrap()
        .iter()
        .any(|event| matches!(event, LocalChatEvent::Session { .. })));
    assert_eq!(store.thread.as_deref(), Some("older-native-thread"));
    println!("legacy_exec platform_conversation=true history_messages=3 resumed_event=false native_mapping_unchanged=true");
}
