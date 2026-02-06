#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingEffort {
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRunnerKind {
    Codex,
    Amp,
    Claude,
}

impl AgentRunnerKind {
    pub fn as_str(self) -> &'static str {
        match self {
            AgentRunnerKind::Codex => "codex",
            AgentRunnerKind::Amp => "amp",
            AgentRunnerKind::Claude => "claude",
        }
    }
}

pub fn parse_agent_runner_kind(value: &str) -> Option<AgentRunnerKind> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("codex") {
        return Some(AgentRunnerKind::Codex);
    }
    if value.eq_ignore_ascii_case("amp") {
        return Some(AgentRunnerKind::Amp);
    }
    if value.eq_ignore_ascii_case("claude") {
        return Some(AgentRunnerKind::Claude);
    }
    None
}

pub fn default_agent_runner_kind() -> AgentRunnerKind {
    AgentRunnerKind::Codex
}

pub fn default_amp_mode() -> &'static str {
    "smart"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_agent_runner_kind_accepts_claude() {
        assert_eq!(
            parse_agent_runner_kind("claude"),
            Some(AgentRunnerKind::Claude)
        );
        assert_eq!(
            parse_agent_runner_kind("ClAuDe"),
            Some(AgentRunnerKind::Claude)
        );
    }
}

impl ThinkingEffort {
    pub const ALL: [ThinkingEffort; 5] = [
        ThinkingEffort::Minimal,
        ThinkingEffort::Low,
        ThinkingEffort::Medium,
        ThinkingEffort::High,
        ThinkingEffort::XHigh,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ThinkingEffort::Minimal => "minimal",
            ThinkingEffort::Low => "low",
            ThinkingEffort::Medium => "medium",
            ThinkingEffort::High => "high",
            ThinkingEffort::XHigh => "xhigh",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ThinkingEffort::Minimal => "Minimal",
            ThinkingEffort::Low => "Low",
            ThinkingEffort::Medium => "Medium",
            ThinkingEffort::High => "High",
            ThinkingEffort::XHigh => "XHigh",
        }
    }
}

pub fn parse_thinking_effort(value: &str) -> Option<ThinkingEffort> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("minimal") {
        return Some(ThinkingEffort::Minimal);
    }
    if value.eq_ignore_ascii_case("low") {
        return Some(ThinkingEffort::Low);
    }
    if value.eq_ignore_ascii_case("medium") {
        return Some(ThinkingEffort::Medium);
    }
    if value.eq_ignore_ascii_case("high") {
        return Some(ThinkingEffort::High);
    }
    if value.eq_ignore_ascii_case("xhigh") {
        return Some(ThinkingEffort::XHigh);
    }
    None
}

#[derive(Clone, Copy, Debug)]
pub struct AgentModelSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub supported_thinking_efforts: &'static [ThinkingEffort],
}

const STANDARD_EFFORTS: &[ThinkingEffort] = &[
    ThinkingEffort::Minimal,
    ThinkingEffort::Low,
    ThinkingEffort::Medium,
    ThinkingEffort::High,
    ThinkingEffort::XHigh,
];

const AGENT_MODELS: &[AgentModelSpec] = &[
    AgentModelSpec {
        id: "gpt-5.3-codex",
        label: "GPT-5.3-Codex",
        supported_thinking_efforts: STANDARD_EFFORTS,
    },
    AgentModelSpec {
        id: "gpt-5.2-codex",
        label: "GPT-5.2-Codex",
        supported_thinking_efforts: STANDARD_EFFORTS,
    },
    AgentModelSpec {
        id: "gpt-5.2",
        label: "GPT-5.2",
        supported_thinking_efforts: STANDARD_EFFORTS,
    },
];

pub fn agent_models() -> &'static [AgentModelSpec] {
    AGENT_MODELS
}

pub fn default_agent_model_id() -> &'static str {
    "gpt-5.2"
}

pub fn default_thinking_effort() -> ThinkingEffort {
    ThinkingEffort::Medium
}

pub fn thinking_effort_supported(model_id: &str, effort: ThinkingEffort) -> bool {
    agent_models()
        .iter()
        .find(|m| m.id == model_id)
        .map(|m| m.supported_thinking_efforts.contains(&effort))
        .unwrap_or(false)
}

pub fn normalize_thinking_effort(model_id: &str, effort: ThinkingEffort) -> ThinkingEffort {
    let Some(spec) = agent_models().iter().find(|m| m.id == model_id) else {
        return default_thinking_effort();
    };

    if spec.supported_thinking_efforts.contains(&effort) {
        return effort;
    }

    let fallback = default_thinking_effort();
    if spec.supported_thinking_efforts.contains(&fallback) {
        return fallback;
    }

    spec.supported_thinking_efforts
        .first()
        .copied()
        .unwrap_or(default_thinking_effort())
}

pub fn agent_model_label(model_id: &str) -> Option<&'static str> {
    agent_models()
        .iter()
        .find(|m| m.id == model_id)
        .map(|m| m.label)
}
