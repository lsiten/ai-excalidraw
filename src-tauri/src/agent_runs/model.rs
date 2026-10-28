use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Created,
    Preparing,
    Running,
    WaitingInput,
    Validating,
    Repairing,
    Succeeded,
    Failed,
    Cancelling,
    Cancelled,
    Interrupted,
    NeedsReconciliation,
}
impl State {
    pub fn terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
    pub fn allows(self, next: Self) -> bool {
        use State::*;
        if self.terminal() {
            return false;
        }
        matches!(
            (self, next),
            (Created, Preparing)
                | (Preparing, Running)
                | (Running, WaitingInput | Validating)
                | (WaitingInput, Running)
                | (Validating, Succeeded | Repairing)
                | (Repairing, Running | Validating)
                | (Cancelling, Cancelled)
                | (Interrupted | NeedsReconciliation, Failed | Cancelled)
        ) || matches!(
            next,
            Failed | Cancelling | Interrupted | NeedsReconciliation
        )
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Backend {
    Api,
    Codex,
    Claude,
    Cursor,
    Pi,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OperationStatus {
    Prepared,
    Applied,
    Failed,
    Unknown,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Info,
    Warning,
    Error,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Running,
    Completed,
    Failed,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum Event {
    Context {
        snapshot: String,
        capabilities: Vec<String>,
    },
    Session {
        session_id: String,
        backend: Backend,
        resumed: bool,
    },
    State {
        state: State,
        message: Option<String>,
    },
    Status {
        message: String,
        level: Level,
    },
    Thinking {
        text: String,
    },
    Message {
        text: String,
        phase: Option<String>,
    },
    Tool {
        call_id: String,
        name: String,
        status: ToolStatus,
        input: Option<String>,
        output: Option<String>,
    },
    Plan {
        steps: Vec<String>,
    },
    Operation {
        operation_id: Uuid,
        status: OperationStatus,
        target_ids: Vec<String>,
    },
    Artifact {
        path: String,
        kind: String,
        verified: bool,
    },
    Usage {
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
    },
    Validation {
        ok: bool,
        message: String,
    },
    Input {
        request_id: Uuid,
        prompt: String,
        choices: Option<Vec<String>>,
        response: Option<String>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Header {
    pub provider_session_id: Option<String>,
    pub id: Uuid,
    pub conversation_id: String,
    pub parent_run_id: Option<Uuid>,
    pub backend: Backend,
    pub project_id: Option<String>,
    pub canvas_id: Option<String>,
    pub context_snapshot: String,
    pub state: State,
    pub created_at: u64,
    pub updated_at: u64,
    pub capabilities: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
    pub run_id: Uuid,
    pub seq: u64,
    pub at: u64,
    pub event: Event,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    pub intended_after_snapshot: Option<String>,
    pub tool: Option<String>,
    pub fingerprint: Option<String>,
    pub base_revision: Option<String>,
    pub before_snapshot: Option<String>,
    pub result: Option<String>,
    pub after_revision: Option<String>,
    pub after_snapshot: Option<String>,
    pub proof: Option<String>,
    pub operation_id: Uuid,
    pub status: OperationStatus,
    pub target_ids: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Run {
    pub header: Header,
    pub events: Vec<Envelope>,
    pub operations: BTreeMap<Uuid, Receipt>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(
    tag = "action",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum Request {
    Create {
        header: Header,
    },
    List {
        project_id: Option<String>,
    },
    Read {
        run_id: Uuid,
        project_id: Option<String>,
    },
    Append {
        run_id: Uuid,
        project_id: Option<String>,
        event: Event,
    },
    PrepareOperation {
        intended_after_snapshot: Option<String>,
        run_id: Uuid,
        project_id: Option<String>,
        operation_id: Uuid,
        tool: Option<String>,
        fingerprint: Option<String>,
        base_revision: Option<String>,
        before_snapshot: Option<String>,
        target_ids: Vec<String>,
    },
    SettleOperation {
        run_id: Uuid,
        project_id: Option<String>,
        operation_id: Uuid,
        status: OperationStatus,
        result: Option<String>,
        after_revision: Option<String>,
        after_snapshot: Option<String>,
    },
    ReconcileOperation {
        run_id: Uuid,
        project_id: Option<String>,
        operation_id: Uuid,
        expected_after_revision: Option<String>,
        current_revision: Option<String>,
        status: OperationStatus,
        proof: Option<String>,
    },
}
