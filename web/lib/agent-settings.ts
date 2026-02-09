"use client"

import type { ThinkingEffort } from "./luban-api"

export type AgentModelSpec = {
  id: string
  label: string
  supportedThinkingEfforts: ThinkingEffort[]
}

export const THINKING_EFFORTS: ThinkingEffort[] = ["minimal", "low", "medium", "high", "xhigh"]

export const AGENT_MODELS: AgentModelSpec[] = [
  {
    id: "gpt-5.3-codex",
    label: "GPT-5.3-Codex",
    supportedThinkingEfforts: THINKING_EFFORTS,
  },
  {
    id: "gpt-5.2-codex",
    label: "GPT-5.2-Codex",
    supportedThinkingEfforts: THINKING_EFFORTS,
  },
  {
    id: "gpt-5.2",
    label: "GPT-5.2",
    supportedThinkingEfforts: THINKING_EFFORTS,
  },
]

// Reason: The droid CLI ignores the `-r` reasoning flag — each model has a
// fixed reasoning level baked in.  Empty array hides the reasoning column.
export const DROID_MODELS: AgentModelSpec[] = [
  { id: "claude-opus-4-6", label: "Claude Opus 4.6", supportedThinkingEfforts: [] },
  { id: "claude-opus-4-5-20251101", label: "Claude Opus 4.5", supportedThinkingEfforts: [] },
  { id: "claude-sonnet-4-5-20250929", label: "Claude Sonnet 4.5", supportedThinkingEfforts: [] },
  { id: "claude-haiku-4-5-20251001", label: "Claude Haiku 4.5", supportedThinkingEfforts: [] },
  { id: "gpt-5.3-codex", label: "GPT-5.3-Codex", supportedThinkingEfforts: [] },
  { id: "gpt-5.2-codex", label: "GPT-5.2-Codex", supportedThinkingEfforts: [] },
  { id: "gpt-5.2", label: "GPT-5.2", supportedThinkingEfforts: [] },
  { id: "gpt-5.1-codex-max", label: "GPT-5.1-Codex-Max", supportedThinkingEfforts: [] },
  { id: "gpt-5.1-codex", label: "GPT-5.1-Codex", supportedThinkingEfforts: [] },
  { id: "gpt-5.1", label: "GPT-5.1", supportedThinkingEfforts: [] },
  { id: "gemini-3-pro-preview", label: "Gemini 3 Pro", supportedThinkingEfforts: [] },
  { id: "gemini-3-flash-preview", label: "Gemini 3 Flash", supportedThinkingEfforts: [] },
  { id: "glm-4.7", label: "GLM-4.7", supportedThinkingEfforts: [] },
  { id: "kimi-k2.5", label: "Kimi K2.5", supportedThinkingEfforts: [] },
]

export function supportedThinkingEffortsForModel(modelId: string | null | undefined): ThinkingEffort[] {
  if (!modelId) return THINKING_EFFORTS
  return (
    AGENT_MODELS.find((m) => m.id === modelId)?.supportedThinkingEfforts ??
    DROID_MODELS.find((m) => m.id === modelId)?.supportedThinkingEfforts ??
    THINKING_EFFORTS
  )
}

/** Runner-aware version: looks up efforts in the correct catalog for the runner. */
export function supportedThinkingEffortsForRunner(
  runner: string | null | undefined,
  modelId: string | null | undefined,
): ThinkingEffort[] {
  if (!modelId) return runner === "droid" ? [] : THINKING_EFFORTS
  if (runner === "droid") {
    return DROID_MODELS.find((m) => m.id === modelId)?.supportedThinkingEfforts ?? []
  }
  return (
    AGENT_MODELS.find((m) => m.id === modelId)?.supportedThinkingEfforts ??
    THINKING_EFFORTS
  )
}

/** Returns true if `modelId` exists in any known model catalog. */
export function isKnownModel(modelId: string | null | undefined): boolean {
  if (!modelId) return false
  return AGENT_MODELS.some((m) => m.id === modelId) || DROID_MODELS.some((m) => m.id === modelId)
}
