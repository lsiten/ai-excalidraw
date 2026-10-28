# Native stream regression fixtures

- `cursor-agent-2026.07.20.jsonl`: unmodified real capture from Multica `server/pkg/agent/testdata/cursor-agent-2026.07.20-stream-json.jsonl` (CLI `2026.07.20-8cc9c0b`). Covers prompt echoes, thinking, read/shell/edit, nested results, and aggregate terminal text.
- `claude-context-exhausted.jsonl`: unmodified real capture from Multica `server/pkg/agent/testdata/claude-code-2.1.220-context-exhausted-resume.jsonl`. Covers `success` subtype with provider failure.
- `claude-partials.jsonl`: constructed protocol fixture, not a paid capture. Uses installed Anthropic SDK `RawContentBlock*Event` shapes and Claude's `stream_event` envelope. Includes separate completed block messages sharing one message id.
- `pi-tools.jsonl`: constructed protocol fixture, not a paid capture. Uses installed Pi 1.4.0 `AgentEvent` and `AssistantMessageEvent` definitions in `@earendil-works/pi-agent-core` / `pi-ai`. Tool start/end and retry shapes are also covered by Multica `server/pkg/agent/pi_test.go`.

Cursor partial-output filtering follows https://cursor.com/docs/cli/reference/output-format.md: timestamped assistant envelopes without `model_call_id` are deltas; buffered flushes repeat them.

Codex stays on `exec --json`. Local `codex-cli 0.157.1` exposes no partial-output flag. Its reasoning and agent messages may arrive only as completed items; tests never manufacture token streaming by splitting snapshots or launching a live model. The retained legacy delta branch is only consumed when a CLI actually emits that event. No app-server transport is launched.
