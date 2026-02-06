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

export function supportedThinkingEffortsForModel(modelId: string | null | undefined): ThinkingEffort[] {
  if (!modelId) return THINKING_EFFORTS
  return AGENT_MODELS.find((m) => m.id === modelId)?.supportedThinkingEfforts ?? THINKING_EFFORTS
}
