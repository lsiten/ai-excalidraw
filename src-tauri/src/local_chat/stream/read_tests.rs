#![expect(
    clippy::unwrap_used,
    reason = "Fixtures and channel delivery must succeed"
)]
use super::*;
use std::time::Duration;
use tokio::{io::AsyncWriteExt, sync::mpsc};

#[tokio::test(flavor = "current_thread")]
async fn fragmented_stdout_delivers_tool_updates_before_terminal_or_eof() {
    // Given a real asynchronous pipe whose writer stays open until progress is read.
    let (mut writer, reader) = tokio::io::duplex(64);
    let (send, mut receive) = mpsc::unbounded_channel();
    let parser = Parser::new(
        ToolId::Pi,
        Box::new(move |event| send.send(event).map_err(|_| ChatError::Channel)),
    );
    let task = tokio::spawn(async move { read(reader, parser, &Mutex::new(None)).await });
    let progress = concat!(
        "{\"type\":\"tool_execution_start\",\"toolCallId\":\"c\",\"toolName\":\"bash\",\"args\":{\"command\":\"echo 你好\"}}\n",
        "{\"type\":\"tool_execution_update\",\"toolCallId\":\"c\",\"toolName\":\"bash\",\"partialResult\":{\"content\":[{\"type\":\"text\",\"text\":\"你\"}]}}\n"
    );
    // When arbitrary byte boundaries split even UTF-8 characters and JSON frames.
    for byte in progress.as_bytes() {
        writer.write_all(&[*byte]).await.unwrap();
    }
    let started = tokio::time::timeout(Duration::from_secs(1), receive.recv())
        .await
        .unwrap()
        .unwrap();
    let updated = tokio::time::timeout(Duration::from_secs(1), receive.recv())
        .await
        .unwrap()
        .unwrap();
    // Then both events arrive while the task is still waiting for its terminal event.
    assert!(matches!(
        started,
        LocalChatEvent::Tool {
            status: ToolStatus::Running,
            input: Some(_),
            output: None,
            ..
        }
    ));
    assert!(
        matches!(updated, LocalChatEvent::Tool { status: ToolStatus::Running, input: Some(_), output: Some(output), .. } if output.contains('你'))
    );
    assert!(!task.is_finished());
    writer.write_all(concat!(
        "{\"type\":\"tool_execution_end\",\"toolCallId\":\"c\",\"toolName\":\"bash\",\"result\":\"你好\",\"isError\":false}\n",
        "{\"type\":\"agent_end\",\"messages\":[]}\n"
    ).as_bytes()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(1), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap()
        .finish()
        .unwrap();
}
