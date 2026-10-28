use serde::Serialize;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ToolStatus {
    Running,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum LocalChatEvent {
    #[serde(rename_all = "camelCase")]
    Session {
        conversation_id: String,
        session_id: String,
        backend: String,
        resumed: bool,
    },
    Phase {
        phase: String,
    },
    Plan {
        explanation: Option<String>,
        steps: Vec<PlanStep>,
    },
    #[serde(rename_all = "camelCase")]
    Usage {
        input_tokens: u64,
        output_tokens: u64,
        cached_input_tokens: u64,
        total_tokens: u64,
    },
    #[serde(rename_all = "camelCase")]
    Input {
        request_id: String,
        kind: String,
        message: String,
        supported: bool,
    },
    Chunk {
        text: String,
    },
    Thinking {
        text: String,
    },
    Tool {
        name: String,
        id: String,
        status: ToolStatus,
        #[serde(skip_serializing_if = "Option::is_none")]
        input: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
    },
    Status {
        message: String,
    },
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct PlanStep {
    pub step: String,
    pub status: String,
}
