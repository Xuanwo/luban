#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ThinkingEffort {
    Low,
    Medium,
    High,
    XHigh,
}

impl ThinkingEffort {
    pub const ALL: [ThinkingEffort; 4] = [
        ThinkingEffort::Low,
        ThinkingEffort::Medium,
        ThinkingEffort::High,
        ThinkingEffort::XHigh,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ThinkingEffort::Low => "low",
            ThinkingEffort::Medium => "medium",
            ThinkingEffort::High => "high",
            ThinkingEffort::XHigh => "xhigh",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ThinkingEffort::Low => "Low",
            ThinkingEffort::Medium => "Medium",
            ThinkingEffort::High => "High",
            ThinkingEffort::XHigh => "XHigh",
        }
    }
}

pub fn parse_thinking_effort(value: &str) -> Option<ThinkingEffort> {
    let value = value.trim();
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

const GPT_52_EFFORTS: &[ThinkingEffort] = &[
    ThinkingEffort::Low,
    ThinkingEffort::Medium,
    ThinkingEffort::High,
    ThinkingEffort::XHigh,
];

const GPT_52_CODEX_EFFORTS: &[ThinkingEffort] = &[
    ThinkingEffort::Low,
    ThinkingEffort::Medium,
    ThinkingEffort::High,
    ThinkingEffort::XHigh,
];

const GPT_51_CODEX_MAX_EFFORTS: &[ThinkingEffort] = &[
    ThinkingEffort::Low,
    ThinkingEffort::Medium,
    ThinkingEffort::High,
    ThinkingEffort::XHigh,
];

const AGENT_MODELS: &[AgentModelSpec] = &[
    AgentModelSpec {
        id: "gpt-5.2",
        label: "GPT-5.2",
        supported_thinking_efforts: GPT_52_EFFORTS,
    },
    AgentModelSpec {
        id: "gpt-5.2-codex",
        label: "GPT-5.2-Codex",
        supported_thinking_efforts: GPT_52_CODEX_EFFORTS,
    },
    AgentModelSpec {
        id: "gpt-5.1-codex-max",
        label: "GPT-5.1-Codex-Max",
        supported_thinking_efforts: GPT_51_CODEX_MAX_EFFORTS,
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
