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
    Droid,
}

impl AgentRunnerKind {
    pub fn as_str(self) -> &'static str {
        match self {
            AgentRunnerKind::Codex => "codex",
            AgentRunnerKind::Amp => "amp",
            AgentRunnerKind::Claude => "claude",
            AgentRunnerKind::Droid => "droid",
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
    if value.eq_ignore_ascii_case("droid") {
        return Some(AgentRunnerKind::Droid);
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

    #[test]
    fn parse_agent_runner_kind_accepts_droid() {
        assert_eq!(
            parse_agent_runner_kind("droid"),
            Some(AgentRunnerKind::Droid)
        );
        assert_eq!(
            parse_agent_runner_kind("DrOiD"),
            Some(AgentRunnerKind::Droid)
        );
    }

    #[test]
    fn thinking_effort_not_supported_for_droid_only_models() {
        // Reason: The droid CLI ignores the `-r` flag, so no effort is "supported".
        // Models shared with the Codex catalog (gpt-5.2-codex, gpt-5.2) are
        // excluded because `find_model_spec` finds the Codex entry first.
        for model_id in [
            "claude-opus-4-6",
            "claude-opus-4-5-20251101",
            "claude-sonnet-4-5-20250929",
            "claude-haiku-4-5-20251001",
            "gpt-5.1-codex-max",
            "gpt-5.1-codex",
            "gpt-5.1",
            "gemini-3-pro-preview",
            "gemini-3-flash-preview",
            "glm-4.7",
            "kimi-k2.5",
        ] {
            assert!(
                !thinking_effort_supported(model_id, ThinkingEffort::High),
                "thinking_effort_supported should return false for droid model {model_id}"
            );
        }
    }

    #[test]
    fn normalize_thinking_effort_falls_back_for_droid_models() {
        // Reason: Droid models have empty supported efforts, so normalize
        // returns the default thinking effort.
        assert_eq!(
            normalize_thinking_effort("claude-opus-4-6", ThinkingEffort::High),
            default_thinking_effort(),
        );
        assert_eq!(
            normalize_thinking_effort("kimi-k2.5", ThinkingEffort::XHigh),
            default_thinking_effort(),
        );
    }

    #[test]
    fn agent_model_label_returns_droid_labels() {
        assert_eq!(
            agent_model_label("claude-opus-4-6"),
            Some("Claude Opus 4.6")
        );
        assert_eq!(
            agent_model_label("claude-opus-4-5-20251101"),
            Some("Claude Opus 4.5")
        );
        assert_eq!(
            agent_model_label("claude-sonnet-4-5-20250929"),
            Some("Claude Sonnet 4.5")
        );
        assert_eq!(
            agent_model_label("claude-haiku-4-5-20251001"),
            Some("Claude Haiku 4.5")
        );
        assert_eq!(agent_model_label("gpt-5.1"), Some("GPT-5.1"));
        assert_eq!(
            agent_model_label("gpt-5.1-codex-max"),
            Some("GPT-5.1-Codex-Max")
        );
        assert_eq!(agent_model_label("glm-4.7"), Some("GLM-4.7"));
        assert_eq!(agent_model_label("kimi-k2.5"), Some("Kimi K2.5"));
    }

    #[test]
    fn agent_model_label_still_works_for_codex_models() {
        assert_eq!(agent_model_label("gpt-5.3-codex"), Some("GPT-5.3-Codex"));
        assert_eq!(agent_model_label("gpt-5.2-codex"), Some("GPT-5.2-Codex"));
        assert_eq!(agent_model_label("gpt-5.2"), Some("GPT-5.2"));
    }

    #[test]
    fn model_valid_for_runner_checks_catalog() {
        assert!(model_valid_for_runner(
            AgentRunnerKind::Codex,
            "gpt-5.3-codex"
        ));
        assert!(model_valid_for_runner(
            AgentRunnerKind::Codex,
            "gpt-5.2-codex"
        ));
        assert!(!model_valid_for_runner(
            AgentRunnerKind::Codex,
            "nonexistent-model"
        ));
        assert!(model_valid_for_runner(
            AgentRunnerKind::Codex,
            "gpt-5.2-codex"
        ));
        assert!(model_valid_for_runner(
            AgentRunnerKind::Droid,
            "claude-opus-4-6"
        ));
        assert!(model_valid_for_runner(AgentRunnerKind::Droid, "gpt-5.2"));
        assert!(model_valid_for_runner(
            AgentRunnerKind::Droid,
            "gpt-5.3-codex"
        ));
        // Amp/Claude have empty catalogs — any model is valid
        assert!(model_valid_for_runner(AgentRunnerKind::Amp, "anything"));
        assert!(model_valid_for_runner(AgentRunnerKind::Claude, "anything"));
    }

    #[test]
    fn runner_for_model_infers_correct_runner() {
        // Codex-only models
        assert_eq!(
            runner_for_model("gpt-5.2-codex"),
            Some(AgentRunnerKind::Codex)
        );
        // Shared models prefer Codex
        assert_eq!(runner_for_model("gpt-5.2"), Some(AgentRunnerKind::Codex));
        // Droid-only models
        assert_eq!(
            runner_for_model("claude-opus-4-6"),
            Some(AgentRunnerKind::Droid)
        );
        assert_eq!(
            runner_for_model("gemini-3-pro-preview"),
            Some(AgentRunnerKind::Droid)
        );
        assert_eq!(runner_for_model("kimi-k2.5"), Some(AgentRunnerKind::Droid));
        // Unknown model
        assert_eq!(runner_for_model("nonexistent-model"), None);
    }

    #[test]
    fn default_model_for_runner_returns_first_catalog_entry() {
        let codex_default = default_model_for_runner(AgentRunnerKind::Codex);
        assert_eq!(codex_default, AGENT_MODELS[0].id);
        let droid_default = default_model_for_runner(AgentRunnerKind::Droid);
        assert_eq!(droid_default, DROID_MODELS[0].id);
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

// Reason: The droid CLI ignores the `-r` reasoning flag — each model has a
// fixed reasoning level baked in.  We set `supported_thinking_efforts` to
// empty so the UI hides the reasoning column for Droid models.
const DROID_MODELS: &[AgentModelSpec] = &[
    AgentModelSpec {
        id: "claude-opus-4-6",
        label: "Claude Opus 4.6",
        supported_thinking_efforts: &[],
    },
    AgentModelSpec {
        id: "claude-opus-4-5-20251101",
        label: "Claude Opus 4.5",
        supported_thinking_efforts: &[],
    },
    AgentModelSpec {
        id: "claude-sonnet-4-5-20250929",
        label: "Claude Sonnet 4.5",
        supported_thinking_efforts: &[],
    },
    AgentModelSpec {
        id: "claude-haiku-4-5-20251001",
        label: "Claude Haiku 4.5",
        supported_thinking_efforts: &[],
    },
    AgentModelSpec {
        id: "gpt-5.3-codex",
        label: "GPT-5.3-Codex",
        supported_thinking_efforts: &[],
    },
    AgentModelSpec {
        id: "gpt-5.2-codex",
        label: "GPT-5.2-Codex",
        supported_thinking_efforts: &[],
    },
    AgentModelSpec {
        id: "gpt-5.2",
        label: "GPT-5.2",
        supported_thinking_efforts: &[],
    },
    AgentModelSpec {
        id: "gpt-5.1-codex-max",
        label: "GPT-5.1-Codex-Max",
        supported_thinking_efforts: &[],
    },
    AgentModelSpec {
        id: "gpt-5.1-codex",
        label: "GPT-5.1-Codex",
        supported_thinking_efforts: &[],
    },
    AgentModelSpec {
        id: "gpt-5.1",
        label: "GPT-5.1",
        supported_thinking_efforts: &[],
    },
    AgentModelSpec {
        id: "gemini-3-pro-preview",
        label: "Gemini 3 Pro",
        supported_thinking_efforts: &[],
    },
    AgentModelSpec {
        id: "gemini-3-flash-preview",
        label: "Gemini 3 Flash",
        supported_thinking_efforts: &[],
    },
    AgentModelSpec {
        id: "glm-4.7",
        label: "GLM-4.7",
        supported_thinking_efforts: &[],
    },
    AgentModelSpec {
        id: "kimi-k2.5",
        label: "Kimi K2.5",
        supported_thinking_efforts: &[],
    },
];

pub fn agent_models() -> &'static [AgentModelSpec] {
    AGENT_MODELS
}

pub fn droid_models() -> &'static [AgentModelSpec] {
    DROID_MODELS
}

pub fn default_agent_model_id() -> &'static str {
    "gpt-5.3-codex"
}

pub fn default_thinking_effort() -> ThinkingEffort {
    ThinkingEffort::Medium
}

/// Look up a model spec by ID across both Codex and Droid catalogs.
fn find_model_spec(model_id: &str) -> Option<&'static AgentModelSpec> {
    AGENT_MODELS
        .iter()
        .chain(DROID_MODELS.iter())
        .find(|m| m.id == model_id)
}

pub fn thinking_effort_supported(model_id: &str, effort: ThinkingEffort) -> bool {
    find_model_spec(model_id)
        .map(|m| m.supported_thinking_efforts.contains(&effort))
        .unwrap_or(false)
}

pub fn normalize_thinking_effort(model_id: &str, effort: ThinkingEffort) -> ThinkingEffort {
    let Some(spec) = find_model_spec(model_id) else {
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
    find_model_spec(model_id).map(|m| m.label)
}

/// Return the model catalog for the given runner kind.
pub fn models_for_runner(runner: AgentRunnerKind) -> &'static [AgentModelSpec] {
    match runner {
        AgentRunnerKind::Codex => AGENT_MODELS,
        AgentRunnerKind::Droid => DROID_MODELS,
        // Amp and Claude don't use the model selector
        _ => &[],
    }
}

/// Return a suitable default model ID for the given runner.
/// Falls back to `default_agent_model_id()` if the runner has no catalog.
pub fn default_model_for_runner(runner: AgentRunnerKind) -> &'static str {
    let catalog = models_for_runner(runner);
    catalog
        .first()
        .map(|m| m.id)
        .unwrap_or(default_agent_model_id())
}

/// Infer which runner owns a given model by checking the Codex and Droid
/// catalogs. Returns `None` for unknown model IDs.
pub fn runner_for_model(model_id: &str) -> Option<AgentRunnerKind> {
    // Reason: Check Codex first so shared models (gpt-5.2, gpt-5.2-codex)
    // prefer the Codex runner, matching the existing `find_model_spec` order.
    if AGENT_MODELS.iter().any(|m| m.id == model_id) {
        return Some(AgentRunnerKind::Codex);
    }
    if DROID_MODELS.iter().any(|m| m.id == model_id) {
        return Some(AgentRunnerKind::Droid);
    }
    None
}

/// Check whether `model_id` exists in the given runner's catalog.
pub fn model_valid_for_runner(runner: AgentRunnerKind, model_id: &str) -> bool {
    let catalog = models_for_runner(runner);
    // Reason: Amp/Claude have empty catalogs — any model is "valid" (ignored).
    catalog.is_empty() || catalog.iter().any(|m| m.id == model_id)
}
