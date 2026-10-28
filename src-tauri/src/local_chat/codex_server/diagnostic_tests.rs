#![expect(
    clippy::unwrap_used,
    reason = "Tests fail immediately on fixture errors"
)]
use super::super::{events::LocalChatEvent, stream::Sink};
use super::{
    diagnostics,
    messages::Messages,
    notifications,
    session::Progress,
    watchdog::{Limits, Watchdog},
};
use serde_json::json;
use std::sync::{Arc, Mutex};
#[test]
fn public_diagnostics_exclude_envelopes_and_sensitive_messages() {
    let envelope = json!({"message":"Rate limited","headers":{"Authorization":"secret-token"},"error":{"data":{"api_key":"secret-key"}}});
    let LocalChatEvent::Status { message } = diagnostics::status(&envelope, true) else {
        panic!("status expected")
    };
    assert!(message.contains("Rate limited"));
    assert!(!message.contains("secret"));
    assert!(!message.contains("headers"));
    for value in [
        json!({"error":{"data":{"token":"secret"}}}),
        json!({"message":"{\"headers\":{\"Authorization\":\"secret\"}}"}),
        json!({"message":"Authorization: Bearer secret"}),
    ] {
        let LocalChatEvent::Status { message } = diagnostics::status(&value, false) else {
            panic!("status expected")
        };
        assert_eq!(message, "Codex reported a warning.");
        assert!(!diagnostics::failure("Codex failed", &value)
            .to_string()
            .contains("secret"));
    }
    let message = diagnostics::failure(
        "Codex failed",
        &json!({"error":{"message":"界".repeat(1000)}}),
    )
    .to_string();
    assert!(message.len() < 1100);
    assert!(message.ends_with("..."));
}
#[test]
fn retry_is_nonfatal_and_terminal_failure_preserves_public_reason_and_state() {
    let mut progress = Progress {
        limits: Limits::default(),
        submitted: true,
        thread: Some("thread".into()),
        turn: Some("turn".into()),
    };
    let mut watchdog = Watchdog::new(&progress.limits);
    let mut messages = Messages::default();
    let events = Arc::new(Mutex::new(Vec::new()));
    let output = events.clone();
    let mut sink: Sink = Box::new(move |event| {
        output.lock().unwrap().push(event);
        Ok(())
    });
    assert!(!notifications::handle(
        "error",
        &json!({"willRetry":true,"error":{"message":"Rate limited","headers":{"token":"secret"}}}),
        &mut sink,
        &mut progress,
        &mut watchdog,
        &mut messages
    )
    .unwrap());
    assert!(
        matches!(events.lock().unwrap().first(),Some(LocalChatEvent::Status{message}) if message.contains("retrying")&&!message.contains("secret"))
    );
    let fatal=notifications::handle("turn/completed",&json!({"turn":{"id":"turn","status":"failed","error":{"message":"Provider unavailable","headers":{"token":"secret"}}}}),&mut sink,&mut progress,&mut watchdog,&mut messages).unwrap_err().to_string();
    assert_eq!(fatal, "Codex turn failed: Provider unavailable");
    let fatal = notifications::handle(
        "error",
        &json!({"willRetry":false,"error":{"message":"Rejected"},"auth":{"token":"secret"}}),
        &mut sink,
        &mut progress,
        &mut watchdog,
        &mut messages,
    )
    .unwrap_err()
    .to_string();
    assert_eq!(fatal, "Codex request failed: Rejected");
    assert_eq!(
        diagnostics::failed_turn(&json!({"status":"interrupted","error":{"message":"Stopped"}}))
            .to_string(),
        "Codex turn was interrupted: Stopped"
    );
}
