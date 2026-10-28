use super::security_tests::fixture;
use super::*;
use serde_json::json;
#[test]
fn preparing_context_updates_provenance_with_redacted_snapshot() -> Result<()> {
    let (dir, h, mut seen) = fixture()?;
    execute(
        dir.path(),
        Request::Append {
            run_id: h.id,
            project_id: h.project_id.clone(),
            event: Event::State {
                state: State::Preparing,
                message: None,
            },
        },
        &mut seen,
    )?;
    let value=execute(dir.path(),Request::Append{run_id:h.id,project_id:h.project_id.clone(),event:Event::Context{snapshot:json!({"skills":[{"name":"draw","revision":"r3"}],"policy":{"write":true},"apiKey":"context-provenance-secret"}).to_string(),capabilities:vec!["canvas.write".into()]}},&mut seen)?;
    let run: Run = serde_json::from_value(value).map_err(|_| "decode")?;
    let snapshot: serde_json::Value =
        serde_json::from_str(&run.header.context_snapshot).map_err(|_| "snapshot")?;
    assert_eq!(snapshot["skills"][0]["revision"], "r3");
    assert_eq!(snapshot["apiKey"], "[REDACTED]");
    assert_eq!(run.header.capabilities, vec!["canvas.write"]);
    let path = dir
        .path()
        .join(".ai-excalidraw/runs")
        .join(format!("{}.json", h.id));
    assert!(!fs::read_to_string(path)
        .map_err(|_| "read")?
        .contains("context-provenance-secret"));
    assert!(matches!(
        run.events.last().map(|e| &e.event),
        Some(Event::Context { .. })
    ));
    Ok(())
}
#[test]
fn context_rejects_wrong_state_invalid_json_and_excess_bounds_without_mutation() -> Result<()> {
    let (dir, h, mut seen) = fixture()?;
    let path = dir
        .path()
        .join(".ai-excalidraw/runs")
        .join(format!("{}.json", h.id));
    for state in [
        State::Created,
        State::Running,
        State::Succeeded,
        State::Preparing,
    ] {
        let mut run = load(&path)?;
        run.header.state = state;
        save(&path, &run)?;
        let before = fs::read(&path).map_err(|_| "read")?;
        let events = if state == State::Preparing {
            vec![
                Event::Context {
                    snapshot: "invalid".into(),
                    capabilities: vec![],
                },
                Event::Context {
                    snapshot: format!("\"{}\"", "a".repeat(65536)),
                    capabilities: vec![],
                },
                Event::Context {
                    snapshot: "{}".into(),
                    capabilities: vec!["x".into(); 257],
                },
            ]
        } else {
            vec![Event::Context {
                snapshot: "{}".into(),
                capabilities: vec![],
            }]
        };
        for event in events {
            assert!(execute(
                dir.path(),
                Request::Append {
                    run_id: h.id,
                    project_id: h.project_id.clone(),
                    event
                },
                &mut seen
            )
            .is_err());
            assert_eq!(before, fs::read(&path).map_err(|_| "read")?);
        }
    }
    Ok(())
}
