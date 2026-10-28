#![expect(
    clippy::unwrap_used,
    reason = "Tests fail immediately on fixture errors"
)]
use super::super::{
    cli::Invocation, error::ChatError, events::LocalChatEvent, process::ProcessControl,
    stream::Sink,
};
use super::{execute, Store};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
#[tokio::test]
async fn human_wait_remains_cancellable_and_total_timeout_interrupts() {
    for cancel_on_wait in [true, false] {
        let data = tempfile::tempdir().unwrap();
        let script = data.path().join("peer.py");
        let trace = data.path().join("trace");
        std::fs::write(&script,r"
import sys,json
trace=open(sys.argv[1],'w',buffering=1)
for line in sys.stdin:
 r=json.loads(line);m=r['method'];trace.write(m+'\n')
 if m=='initialized':continue
 if m=='turn/interrupt':break
 result={}
 if m=='thread/start':result={'thread':{'id':'thread'}}
 if m=='turn/start':result={'turn':{'id':'turn'}}
 print(json.dumps({'id':r['id'],'result':result}),flush=True)
 if m=='turn/start':print(json.dumps({'method':'item/started','params':{'item':{'id':'human','type':'mcpToolCall','server':'ai-excalidraw','tool':'remember_project_fact','arguments':{}}}}),flush=True)
").unwrap();
        let mut command = tokio::process::Command::new("/usr/bin/python3");
        command
            .arg(script)
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
            process: Arc::default(),
            duration: Duration::from_secs(2),
        };
        let waiting = Arc::new(Mutex::new(false));
        let seen = waiting.clone();
        let mut sink: Sink = Box::new(move |event| {
            if matches!(event,LocalChatEvent::Phase{phase} if phase=="waiting_input") {
                *seen.lock().unwrap() = true;
                if cancel_on_wait {
                    tx.send_replace(true);
                }
            }
            Ok(())
        });
        let mut store = Store::open(data.path(), None, None, None).unwrap();
        let result = execute(invocation, "fixture", &mut sink, &mut control, &mut store).await;
        assert!(*waiting.lock().unwrap());
        if cancel_on_wait {
            assert!(matches!(result, Err(ChatError::Cancelled)));
        } else {
            assert!(matches!(result, Err(ChatError::Timeout)));
        }
        let trace = std::fs::read_to_string(trace).unwrap();
        assert_eq!(trace.lines().filter(|s| *s == "turn/start").count(), 1);
        assert!(trace.contains("turn/interrupt"));
        println!("human_wait cancel={cancel_on_wait} total_timeout={} interrupt_sent=true child_reaped=true",!cancel_on_wait);
    }
}
