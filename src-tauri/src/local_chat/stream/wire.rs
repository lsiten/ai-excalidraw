use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

// Wire values are parsed at this boundary. Arbitrary tool payloads become bounded strings.
#[derive(Default, Deserialize)]
#[serde(default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Native CLI wire fields preserve independent provider error and retry flags"
)]
pub(super) struct Event {
    #[serde(rename = "type")]
    pub kind: String,
    pub timestamp_ms: Option<u64>,
    pub model_call_id: Option<String>,
    #[serde(alias = "itemId")]
    pub item_id: String,
    #[serde(rename = "event")]
    pub partial: Option<PartialEvent>,
    pub status: Value,
    pub terminal_reason: String,
    pub subtype: String,
    pub message: Value,
    pub item: Option<Item>,
    pub usage: Option<Usage>,
    pub delta: Value,
    pub text: String,
    pub result: Value,
    pub is_error: bool,
    pub errors: Vec<String>,
    pub error: Value,
    pub call_id: String,
    pub tool_call: BTreeMap<String, Value>,
    pub tool_name: String,
    pub tool_id: String,
    pub output: String,
    pub parameters: Value,
    #[serde(rename = "assistantMessageEvent")]
    pub assistant_delta: Option<Delta>,
    #[serde(rename = "toolCallId")]
    pub tool_call_id: String,
    #[serde(rename = "toolName")]
    pub pi_tool_name: String,
    pub args: Value,
    #[serde(rename = "isError")]
    pub pi_is_error: bool,
    pub messages: Vec<Message>,
    #[serde(rename = "willRetry")]
    pub will_retry: bool,
    pub success: bool,
    #[serde(rename = "finalError")]
    pub final_error: String,
    #[serde(rename = "partialResult")]
    pub partial_result: Value,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub(super) struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub(super) struct PartialEvent {
    #[serde(rename = "type")]
    pub kind: String,
    pub index: usize,
    pub message: Message,
    pub content_block: Block,
    pub delta: PartialDelta,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub(super) struct PartialDelta {
    #[serde(rename = "type")]
    pub kind: String,
    pub text: String,
    pub thinking: String,
    pub partial_json: String,
}

#[derive(Default, Deserialize, serde::Serialize)]
#[serde(default)]
pub(super) struct PlanStep {
    pub text: String,
    pub completed: bool,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub(super) struct Message {
    pub id: String,
    pub role: String,
    pub content: Vec<Block>,
    #[serde(rename = "stopReason")]
    pub stop_reason: String,
    #[serde(rename = "errorMessage")]
    pub failure: String,
}

#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Block {
    #[serde(rename = "type")]
    pub kind: String,
    pub text: String,
    pub thinking: String,
    pub id: String,
    pub name: String,
    pub tool_use_id: String,
    #[serde(alias = "arguments")]
    pub input: Value,
    pub content: Value,
    pub is_error: bool,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub(super) struct Delta {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(rename = "delta")]
    pub fragment: String,
    #[serde(rename = "contentIndex")]
    pub content_index: usize,
    pub content: String,
    #[serde(rename = "toolCall")]
    pub tool_call: Option<Block>,
    pub partial: Message,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub(super) struct Item {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub text: String,
    pub message: String,
    pub status: String,
    pub command: String,
    #[serde(alias = "aggregatedOutput")]
    pub aggregated_output: String,
    pub server: String,
    pub tool: String,
    pub arguments: Value,
    pub result: Value,
    pub error: Value,
    #[serde(alias = "exitCode")]
    pub exit_code: Option<i32>,
    pub changes: Value,
    pub items: Vec<PlanStep>,
    pub query: String,
}
