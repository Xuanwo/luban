use super::attachments::AttachmentRef;
use crate::ThinkingEffort;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AgentRunConfig {
    #[serde(default = "crate::default_agent_runner_kind")]
    pub runner: crate::AgentRunnerKind,
    pub model_id: String,
    pub thinking_effort: ThinkingEffort,
    #[serde(default)]
    pub amp_mode: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct QueuedPrompt {
    pub id: u64,
    pub text: String,
    pub attachments: Vec<AttachmentRef>,
    pub run_config: AgentRunConfig,
}
