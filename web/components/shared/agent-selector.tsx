"use client"

import type { AgentRunnerKind, ThinkingEffort } from "@/lib/luban-api"

import { useCallback, useMemo, useRef, useState } from "react"
import Image from "next/image"
import { ChevronDown, Settings } from "lucide-react"

import { cn } from "@/lib/utils"
import { AGENT_MODELS, DROID_MODELS, supportedThinkingEffortsForModel, supportedThinkingEffortsForRunner, isKnownModel } from "@/lib/agent-settings"
import { agentModelLabel, thinkingEffortLabel } from "@/lib/conversation-ui"
import { UnifiedProviderLogo } from "@/components/shared/unified-provider-logo"

const AMP_MARK_URL = "/logos/amp.svg"

const DROPDOWN_GAP = 4

// Reason: Compute fixed-position coordinates and max-height for the dropdown
// based on the trigger button's viewport rect. This avoids overflow clipping
// when the dropdown is inside a modal or other constrained container.
function useDropdownPosition(direction: "top" | "bottom") {
  const triggerRef = useRef<HTMLButtonElement>(null)
  const [pos, setPos] = useState<React.CSSProperties>({})

  const recompute = useCallback(() => {
    const rect = triggerRef.current?.getBoundingClientRect()
    if (!rect) return
    if (direction === "top") {
      const maxH = rect.top - DROPDOWN_GAP
      setPos({
        position: "fixed",
        left: rect.left,
        bottom: window.innerHeight - rect.top + DROPDOWN_GAP,
        maxHeight: Math.max(maxH, 120),
      })
    } else {
      const maxH = window.innerHeight - rect.bottom - DROPDOWN_GAP
      setPos({
        position: "fixed",
        left: rect.left,
        top: rect.bottom + DROPDOWN_GAP,
        maxHeight: Math.max(maxH, 120),
      })
    }
  }, [direction])

  return { triggerRef, pos, recompute }
}

function AmpMark({ className }: { className?: string }) {
  return (
    <Image
      data-agent-runner-icon="amp"
      src={AMP_MARK_URL}
      alt=""
      aria-hidden="true"
      className={cn("inline-block", className)}
      width={14}
      height={14}
    />
  )
}

export function CodexAgentSelector({
  testId = "codex-agent-selector",
  modelId,
  thinkingEffort,
  onChangeModelId,
  onChangeThinkingEffort,
  defaultModelId,
  defaultThinkingEffort,
  onOpenAgentSettings,
  disabled = false,
  dropdownPosition = "bottom",
  className,
}: {
  testId?: string
  modelId: string | null | undefined
  thinkingEffort: ThinkingEffort | null | undefined
  onChangeModelId: (modelId: string) => void
  onChangeThinkingEffort: (effort: ThinkingEffort) => void
  defaultModelId?: string | null
  defaultThinkingEffort?: ThinkingEffort | null
  onOpenAgentSettings?: (agentId: string, filePath?: string) => void
  disabled?: boolean
  dropdownPosition?: "top" | "bottom"
  className?: string
}) {
  const [open, setOpen] = useState(false)
  const [tempModelId, setTempModelId] = useState<string | null>(null)
  const { triggerRef, pos, recompute } = useDropdownPosition(dropdownPosition)

  const currentModelId = modelId ?? ""
  const currentEffort = thinkingEffort ?? null

  const validCodexModelId = isKnownModel(modelId) ? modelId : null
  const displayName = useMemo(() => {
    const model = agentModelLabel(validCodexModelId)
    const effort = thinkingEffortLabel(thinkingEffort)
    if (model === "Model" || effort === "Effort") return "Codex"
    return `${model} · ${effort}`
  }, [validCodexModelId, thinkingEffort])

  const panelModelId = tempModelId ?? currentModelId
  const effortOptions = useMemo(() => supportedThinkingEffortsForModel(panelModelId), [panelModelId])

  const close = () => {
    setOpen(false)
    setTempModelId(null)
  }

  const apply = (nextModelId: string, nextEffort: ThinkingEffort) => {
    if (nextModelId && nextModelId !== currentModelId) onChangeModelId(nextModelId)
    if (nextEffort !== currentEffort && nextEffort != null) onChangeThinkingEffort(nextEffort)
    close()
  }

  return (
    <div className={cn("relative", className)}>
      <button
        ref={triggerRef}
        data-testid={testId}
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => {
          if (disabled) return
          const next = !open
          setOpen(next)
          setTempModelId(null)
          if (next) recompute()
        }}
        disabled={disabled}
        className={cn(
          "inline-flex items-center gap-1.5 px-2 py-1 rounded text-xs transition-colors",
          "text-muted-foreground hover:text-foreground hover:bg-muted",
          open && "bg-muted text-foreground",
          disabled && "opacity-60 cursor-default hover:bg-transparent hover:text-muted-foreground",
        )}
      >
        <UnifiedProviderLogo providerId="openai" className="w-3.5 h-3.5" />
        <span className="whitespace-nowrap">{displayName}</span>
        <ChevronDown className={cn("w-3 h-3 transition-transform", open && "rotate-180")} />
      </button>

      {open && !disabled && (
        <>
          <div data-testid={`${testId}-overlay`} className="fixed inset-0 z-40" onClick={() => close()} />
          <div
            className="bg-popover border border-border rounded-lg shadow-xl z-50 flex flex-col overflow-hidden"
            style={pos}
          >
            <div className="flex divide-x divide-border flex-1 min-h-0 [&>div]:overflow-y-auto">
              <div className="p-1">
                <div className="px-2.5 py-1.5 text-[10px] uppercase tracking-wider text-muted-foreground font-medium">
                  Agent
                </div>
                <button
                  onMouseDown={(e) => e.preventDefault()}
                  className="w-full flex items-center gap-2 px-2.5 py-1.5 text-left text-xs transition-colors rounded-md whitespace-nowrap bg-primary/10 text-primary"
                >
                  <UnifiedProviderLogo providerId="openai" className="w-3.5 h-3.5 flex-shrink-0" />
                  Codex
                </button>
              </div>

              <div className="p-1">
                <div className="px-2.5 py-1.5 text-[10px] uppercase tracking-wider text-muted-foreground font-medium">
                  Model
                </div>
                {AGENT_MODELS.map((m) => {
                  const selected = m.id === panelModelId || (panelModelId === "" && m.id === currentModelId)
                  const isDefault = defaultModelId != null && m.id === defaultModelId
                  return (
                    <div key={m.id} className="relative group">
                      <button
                        onMouseDown={(e) => e.preventDefault()}
                        onClick={() => setTempModelId(m.id)}
                        className={cn(
                          "w-full flex items-center px-2.5 py-1.5 text-left text-xs transition-colors rounded-md whitespace-nowrap",
                          selected ? "bg-primary/10 text-primary" : "text-foreground hover:bg-accent",
                        )}
                      >
                        <span className="pr-10">{m.label}</span>
                      </button>
                      {isDefault && (
                        <div className="absolute right-1 top-1/2 -translate-y-1/2 flex items-center gap-1 px-1.5 py-0.5 opacity-0 pointer-events-none group-hover:opacity-100 transition-opacity bg-popover/95 rounded">
                          <span className="text-[10px] text-muted-foreground pointer-events-none select-none">
                            default
                          </span>
                          {onOpenAgentSettings && (
                            <button
                              onMouseDown={(e) => e.preventDefault()}
                              onClick={(e) => {
                                e.stopPropagation()
                                close()
                                onOpenAgentSettings("codex", "config.toml")
                              }}
                              className="p-0.5 rounded text-muted-foreground hover:text-foreground hover:bg-accent transition-colors pointer-events-auto"
                              title="Edit Codex defaults"
                            >
                              <Settings className="w-3 h-3" />
                            </button>
                          )}
                        </div>
                      )}
                    </div>
                  )
                })}
              </div>

              <div className="p-1">
                <div className="px-2.5 py-1.5 text-[10px] uppercase tracking-wider text-muted-foreground font-medium">
                  Reasoning
                </div>
                {effortOptions.map((effort) => {
                  const selected = effort === (currentEffort ?? "")
                  const isDefault = defaultThinkingEffort != null && effort === defaultThinkingEffort
                  return (
                    <div key={effort} className="relative group">
                      <button
                        onMouseDown={(e) => e.preventDefault()}
                        onClick={() => apply(panelModelId, effort)}
                        className={cn(
                          "w-full flex items-center px-2.5 py-1.5 text-left text-xs transition-colors rounded-md whitespace-nowrap",
                          selected ? "bg-primary/10 text-primary" : "text-foreground hover:bg-accent",
                        )}
                      >
                        <span className="pr-10">{thinkingEffortLabel(effort)}</span>
                      </button>
                      {isDefault && (
                        <div className="absolute right-1 top-1/2 -translate-y-1/2 flex items-center gap-1 px-1.5 py-0.5 opacity-0 pointer-events-none group-hover:opacity-100 transition-opacity bg-popover/95 rounded">
                          <span className="text-[10px] text-muted-foreground pointer-events-none select-none">
                            default
                          </span>
                          {onOpenAgentSettings && (
                            <button
                              onMouseDown={(e) => e.preventDefault()}
                              onClick={(e) => {
                                e.stopPropagation()
                                close()
                                onOpenAgentSettings("codex", "config.toml")
                              }}
                              className="p-0.5 rounded text-muted-foreground hover:text-foreground hover:bg-accent transition-colors pointer-events-auto"
                              title="Edit Codex defaults"
                            >
                              <Settings className="w-3 h-3" />
                            </button>
                          )}
                        </div>
                      )}
                    </div>
                  )
                })}
              </div>
            </div>
          </div>
        </>
      )}
    </div>
  )
}

export type AmpMode = "smart" | "rush" | null

export function AgentSelector({
  testId = "agent-selector",
  modelId,
  thinkingEffort,
  onChangeModelId,
  onChangeThinkingEffort,
  defaultModelId,
  defaultThinkingEffort,
  onOpenAgentSettings,
  disabled = false,
  dropdownPosition = "bottom",
  className,
  defaultRunner,
  defaultAmpMode,
  runner,
  onChangeRunner,
  ampMode,
  onChangeAmpMode,
  codexEnabled = true,
  ampEnabled = true,
  claudeEnabled = true,
  droidEnabled = true,
  runnerDefaultModels,
}: {
  testId?: string
  modelId: string | null | undefined
  thinkingEffort: ThinkingEffort | null | undefined
  onChangeModelId: (modelId: string) => void
  onChangeThinkingEffort: (effort: ThinkingEffort) => void
  defaultModelId?: string | null
  defaultThinkingEffort?: ThinkingEffort | null
  onOpenAgentSettings?: (agentId: string, filePath?: string) => void
  disabled?: boolean
  dropdownPosition?: "top" | "bottom"
  className?: string
  defaultRunner: AgentRunnerKind | null | undefined
  defaultAmpMode: string | null | undefined
  runner: AgentRunnerKind | null | undefined
  onChangeRunner: (runner: AgentRunnerKind) => void
  ampMode: string | null | undefined
  onChangeAmpMode: (mode: AmpMode) => void
  codexEnabled?: boolean
  ampEnabled?: boolean
  claudeEnabled?: boolean
  droidEnabled?: boolean
  runnerDefaultModels?: Record<string, string> | null
}) {
  const resolvedDefaultRunner: AgentRunnerKind = defaultRunner ?? "codex"
  const resolvedRunner: AgentRunnerKind = runner ?? resolvedDefaultRunner
  const isAmp = resolvedRunner === "amp"
  const isClaude = resolvedRunner === "claude"
  const isDroid = resolvedRunner === "droid"
  const resolvedDefaultAmpMode: AmpMode =
    defaultAmpMode === "rush" ? "rush" : defaultAmpMode === "smart" ? "smart" : null
  const resolvedAmpMode: AmpMode = ampMode === "rush" ? "rush" : ampMode === "smart" ? "smart" : null

  // Reason: If the persisted modelId was removed from catalogs (e.g. gpt-5.3-codex),
  // treat it as unset so we show the agent name instead of a raw invalid ID.
  const validModelId = isKnownModel(modelId) ? modelId : null

  const displayName = useMemo(() => {
    if (isAmp) {
      if (resolvedAmpMode === "rush") return "Amp · Rush"
      if (resolvedAmpMode === "smart") return "Amp · Smart"
      return "Amp"
    }
    if (isClaude) return "Claude"
    const model = agentModelLabel(validModelId)
    const effort = thinkingEffortLabel(thinkingEffort)
    if (isDroid) {
      if (model === "Model") return "Droid"
      const droidEfforts = supportedThinkingEffortsForRunner("droid", validModelId)
      if (droidEfforts.length === 0) return model
      if (effort === "Effort") return model
      return `${model} · ${effort}`
    }
    if (model === "Model" || effort === "Effort") return "Codex"
    return `${model} · ${effort}`
  }, [isAmp, isClaude, isDroid, validModelId, resolvedAmpMode, thinkingEffort])

  const icon = isAmp ? (
    <AmpMark className="w-3.5 h-3.5" />
  ) : isClaude ? (
    <UnifiedProviderLogo providerId="anthropic" className="w-3.5 h-3.5" />
  ) : isDroid ? (
    <UnifiedProviderLogo providerId="factory" className="w-3.5 h-3.5" />
  ) : (
    <UnifiedProviderLogo providerId="openai" className="w-3.5 h-3.5" />
  )

  const [open, setOpen] = useState(false)
  const [tempModelId, setTempModelId] = useState<string | null>(null)
  const [tempRunner, setTempRunner] = useState<AgentRunnerKind>(resolvedRunner)
  const { triggerRef, pos, recompute } = useDropdownPosition(dropdownPosition)

  const currentModelId = modelId ?? ""
  const currentEffort = thinkingEffort ?? null

  const panelModelId = tempModelId ?? currentModelId
  // Reason: Use runner-aware lookup so Droid models (which have fixed reasoning)
  // correctly return an empty list, hiding the Reasoning column.
  const effortOptions = useMemo(
    () => supportedThinkingEffortsForRunner(tempRunner, panelModelId),
    [tempRunner, panelModelId],
  )

  const close = () => {
    setOpen(false)
    setTempModelId(null)
    setTempRunner(resolvedRunner)
  }

  const apply = (nextModelId: string, nextEffort: ThinkingEffort) => {
    if (nextModelId && nextModelId !== currentModelId) onChangeModelId(nextModelId)
    if (nextEffort !== currentEffort && nextEffort != null) onChangeThinkingEffort(nextEffort)
    close()
  }

  const selectRunner = (next: AgentRunnerKind) => {
    setTempRunner(next)
    setTempModelId(null)
    onChangeRunner(next)
  }

  const noAgentsEnabled = !codexEnabled && !ampEnabled && !claudeEnabled && !droidEnabled

  if (noAgentsEnabled) {
    return (
      <div className={cn("relative", className)}>
        <button
          data-testid={testId}
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => onOpenAgentSettings?.("codex")}
          className={cn(
            "inline-flex items-center gap-1.5 px-2 py-1 rounded text-xs transition-colors",
            "text-muted-foreground hover:text-foreground hover:bg-muted",
          )}
          title="Enable agents in Settings"
        >
          <Settings className="w-3.5 h-3.5" />
          <span className="whitespace-nowrap">No agent enabled</span>
        </button>
      </div>
    )
  }

  return (
    <div className={cn("relative", className)}>
      <button
        ref={triggerRef}
        data-testid={testId}
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => {
          if (disabled) return
          const nextOpen = !open
          setOpen(nextOpen)
          if (nextOpen) {
            setTempRunner(resolvedRunner)
            setTempModelId(null)
            recompute()
          }
        }}
        disabled={disabled}
        className={cn(
          "inline-flex items-center gap-1.5 px-2 py-1 rounded text-xs transition-colors",
          "text-muted-foreground hover:text-foreground hover:bg-muted",
          open && "bg-muted text-foreground",
          disabled && "opacity-60 cursor-default hover:bg-transparent hover:text-muted-foreground",
        )}
      >
        {icon}
        <span className="whitespace-nowrap">{displayName}</span>
        <ChevronDown className={cn("w-3 h-3 transition-transform", open && "rotate-180")} />
      </button>

      {open && !disabled && (
        <>
          <div data-testid={`${testId}-overlay`} className="fixed inset-0 z-40" onClick={() => close()} />
          <div
            className="bg-popover border border-border rounded-lg shadow-xl z-50 flex flex-col overflow-hidden"
            style={pos}
          >
            <div className="flex divide-x divide-border flex-1 min-h-0 [&>div]:overflow-y-auto">
              <div className="p-1">
                <div className="px-2.5 py-1.5 text-[10px] uppercase tracking-wider text-muted-foreground font-medium">
                  Agent
                </div>

                {([
                  {
                    id: "codex" as const,
                    label: "Codex",
                    icon: <UnifiedProviderLogo providerId="openai" className="w-3.5 h-3.5 flex-shrink-0" />,
                    enabled: codexEnabled,
                  },
                  { id: "amp" as const, label: "Amp", icon: <AmpMark className="w-3.5 h-3.5 flex-shrink-0" />, enabled: ampEnabled },
                  {
                    id: "claude" as const,
                    label: "Claude",
                    icon: <UnifiedProviderLogo providerId="anthropic" className="w-3.5 h-3.5 flex-shrink-0" />,
                    enabled: claudeEnabled,
                  },
                  {
                    id: "droid" as const,
                    label: "Droid",
                    icon: <UnifiedProviderLogo providerId="factory" className="w-3.5 h-3.5 flex-shrink-0" />,
                    enabled: droidEnabled,
                  },
                ] as const)
                  .filter((opt) => opt.enabled)
                  .map((opt) => {
                  const selected = opt.id === tempRunner
                  return (
                    <div key={opt.id}>
                      <button
                        data-testid={`agent-runner-option-${opt.id}`}
                        onMouseDown={(e) => e.preventDefault()}
                        onClick={() => selectRunner(opt.id)}
                        className={cn(
                          "w-full flex items-center gap-2 px-2.5 py-1.5 text-left text-xs transition-colors rounded-md whitespace-nowrap",
                          selected ? "bg-primary/10 text-primary" : "text-foreground hover:bg-accent",
                        )}
                      >
                        {opt.icon}
                        {opt.label}
                      </button>
                    </div>
                  )
                })}
              </div>

              {tempRunner === "codex" ? (
                <>
                  <div className="p-1">
                    <div className="px-2.5 py-1.5 text-[10px] uppercase tracking-wider text-muted-foreground font-medium">
                      Model
                    </div>
                    {AGENT_MODELS.map((m) => {
                      const selected = m.id === panelModelId || (panelModelId === "" && m.id === currentModelId)
                      const isDefault = defaultModelId != null && m.id === defaultModelId
                      return (
                        <div key={m.id} className="relative group">
                          <button
                            onMouseDown={(e) => e.preventDefault()}
                            onClick={() => setTempModelId(m.id)}
                            className={cn(
                              "w-full flex items-center px-2.5 py-1.5 text-left text-xs transition-colors rounded-md whitespace-nowrap",
                              selected ? "bg-primary/10 text-primary" : "text-foreground hover:bg-accent",
                            )}
                          >
                            <span className="pr-10">{m.label}</span>
                          </button>
                          {isDefault && (
                            <div className="absolute right-1 top-1/2 -translate-y-1/2 flex items-center gap-1 px-1.5 py-0.5 opacity-0 pointer-events-none group-hover:opacity-100 transition-opacity bg-popover/95 rounded">
                              <span className="text-[10px] text-muted-foreground pointer-events-none select-none">
                                default
                              </span>
                              {onOpenAgentSettings && (
                                <button
                                  onMouseDown={(e) => e.preventDefault()}
                                  onClick={(e) => {
                                    e.stopPropagation()
                                    close()
                                    onOpenAgentSettings("codex", "config.toml")
                                  }}
                                  className="p-0.5 rounded text-muted-foreground hover:text-foreground hover:bg-accent transition-colors pointer-events-auto"
                                  title="Edit Codex defaults"
                                >
                                  <Settings className="w-3 h-3" />
                                </button>
                              )}
                            </div>
                          )}
                        </div>
                      )
                    })}
                  </div>

                  <div className="p-1">
                    <div className="px-2.5 py-1.5 text-[10px] uppercase tracking-wider text-muted-foreground font-medium">
                      Reasoning
                    </div>
                    {effortOptions.map((effort) => {
                      const selected = effort === (currentEffort ?? "")
                      const isDefault = defaultThinkingEffort != null && effort === defaultThinkingEffort
                      return (
                        <div key={effort} className="relative group">
                          <button
                            onMouseDown={(e) => e.preventDefault()}
                            onClick={() => apply(panelModelId, effort)}
                            className={cn(
                              "w-full flex items-center px-2.5 py-1.5 text-left text-xs transition-colors rounded-md whitespace-nowrap",
                              selected ? "bg-primary/10 text-primary" : "text-foreground hover:bg-accent",
                            )}
                          >
                            <span className="pr-10">{thinkingEffortLabel(effort)}</span>
                          </button>
                          {isDefault && (
                            <div className="absolute right-1 top-1/2 -translate-y-1/2 flex items-center gap-1 px-1.5 py-0.5 opacity-0 pointer-events-none group-hover:opacity-100 transition-opacity bg-popover/95 rounded">
                              <span className="text-[10px] text-muted-foreground pointer-events-none select-none">
                                default
                              </span>
                              {onOpenAgentSettings && (
                                <button
                                  onMouseDown={(e) => e.preventDefault()}
                                  onClick={(e) => {
                                    e.stopPropagation()
                                    close()
                                    onOpenAgentSettings("codex", "config.toml")
                                  }}
                                  className="p-0.5 rounded text-muted-foreground hover:text-foreground hover:bg-accent transition-colors pointer-events-auto"
                                  title="Edit Codex defaults"
                                >
                                  <Settings className="w-3 h-3" />
                                </button>
                              )}
                            </div>
                          )}
                        </div>
                      )
                    })}
                  </div>
                </>
              ) : tempRunner === "amp" ? (
                <div className="p-1">
                  <div className="px-2.5 py-1.5 text-[10px] uppercase tracking-wider text-muted-foreground font-medium">
                    Mode
                  </div>
                  {([
                    { id: "smart" as const, label: "Smart" },
                    { id: "rush" as const, label: "Rush" },
                  ] as const).map((opt) => {
                    const selected = opt.id === (resolvedAmpMode ?? resolvedDefaultAmpMode)
                    const isDefault = resolvedDefaultAmpMode != null && opt.id === resolvedDefaultAmpMode
                    return (
                      <div key={opt.id} className="relative group">
                        <button
                          onMouseDown={(e) => e.preventDefault()}
                          onClick={() => {
                            onChangeAmpMode(opt.id)
                            close()
                          }}
                          className={cn(
                            "w-full flex items-center px-2.5 py-1.5 text-left text-xs transition-colors rounded-md whitespace-nowrap",
                            selected ? "bg-primary/10 text-primary" : "text-foreground hover:bg-accent",
                          )}
                        >
                          <span className="pr-10">{opt.label}</span>
                        </button>
                        {isDefault && (
                          <div className="absolute right-1 top-1/2 -translate-y-1/2 flex items-center gap-1 px-1.5 py-0.5 opacity-0 pointer-events-none group-hover:opacity-100 transition-opacity bg-popover/95 rounded">
                            <span className="text-[10px] text-muted-foreground pointer-events-none select-none">
                              default
                            </span>
                            {onOpenAgentSettings && (
                              <button
                                onMouseDown={(e) => e.preventDefault()}
                                onClick={(e) => {
                                  e.stopPropagation()
                                  close()
                                  onOpenAgentSettings("amp")
                                }}
                                className="p-0.5 rounded text-muted-foreground hover:text-foreground hover:bg-accent transition-colors pointer-events-auto"
                                title="Edit Amp defaults"
                              >
                                <Settings className="w-3 h-3" />
                              </button>
                            )}
                          </div>
                        )}
                      </div>
                    )
                  })}
                </div>
              ) : tempRunner === "droid" ? (
                <>
                  <div className="p-1">
                    <div className="px-2.5 py-1.5 text-[10px] uppercase tracking-wider text-muted-foreground font-medium">
                      Model
                    </div>
                    {DROID_MODELS.map((m) => {
                      const selected = m.id === panelModelId || (panelModelId === "" && m.id === currentModelId)
                      // Reason: Use the per-runner default so the badge shows
                      // the user's last-chosen model for this runner.
                      const droidDefault = runnerDefaultModels?.["droid"]
                      const isDefault = droidDefault != null && m.id === droidDefault
                      // Reason: Use runner-aware lookup so models shared with
                      // Codex (gpt-5.2, gpt-5.2-codex) resolve to the Droid
                      // entry (empty efforts) and apply directly on click.
                      const modelEfforts = supportedThinkingEffortsForRunner("droid", m.id)
                      return (
                        <div key={m.id} className="relative group">
                          <button
                            onMouseDown={(e) => e.preventDefault()}
                            onClick={() => {
                              if (modelEfforts.length === 0) {
                                // Reason: No reasoning column for this model, apply directly.
                                apply(m.id, currentEffort ?? "medium")
                              } else {
                                setTempModelId(m.id)
                              }
                            }}
                            className={cn(
                              "w-full flex items-center px-2.5 py-1.5 text-left text-xs transition-colors rounded-md whitespace-nowrap",
                              selected ? "bg-primary/10 text-primary" : "text-foreground hover:bg-accent",
                            )}
                          >
                            <span className="pr-10">{m.label}</span>
                          </button>
                          {isDefault && (
                            <div className="absolute right-1 top-1/2 -translate-y-1/2 flex items-center gap-1 px-1.5 py-0.5 opacity-0 pointer-events-none group-hover:opacity-100 transition-opacity bg-popover/95 rounded">
                              <span className="text-[10px] text-muted-foreground pointer-events-none select-none">
                                default
                              </span>
                              {onOpenAgentSettings && (
                                <button
                                  onMouseDown={(e) => e.preventDefault()}
                                  onClick={(e) => {
                                    e.stopPropagation()
                                    close()
                                    onOpenAgentSettings("droid", "settings.json")
                                  }}
                                  className="p-0.5 rounded text-muted-foreground hover:text-foreground hover:bg-accent transition-colors pointer-events-auto"
                                  title="Edit Droid defaults"
                                >
                                  <Settings className="w-3 h-3" />
                                </button>
                              )}
                            </div>
                          )}
                        </div>
                      )
                    })}
                  </div>

                  {effortOptions.length > 0 && (
                    <div className="p-1">
                      <div className="px-2.5 py-1.5 text-[10px] uppercase tracking-wider text-muted-foreground font-medium">
                        Reasoning
                      </div>
                      {effortOptions.map((effort) => {
                        const selected = effort === (currentEffort ?? "")
                        const isDefault = defaultThinkingEffort != null && effort === defaultThinkingEffort
                        return (
                          <div key={effort} className="relative group">
                            <button
                              onMouseDown={(e) => e.preventDefault()}
                              onClick={() => apply(panelModelId, effort)}
                              className={cn(
                                "w-full flex items-center px-2.5 py-1.5 text-left text-xs transition-colors rounded-md whitespace-nowrap",
                                selected ? "bg-primary/10 text-primary" : "text-foreground hover:bg-accent",
                              )}
                            >
                              <span className="pr-10">{thinkingEffortLabel(effort)}</span>
                            </button>
                            {isDefault && (
                              <div className="absolute right-1 top-1/2 -translate-y-1/2 flex items-center gap-1 px-1.5 py-0.5 opacity-0 pointer-events-none group-hover:opacity-100 transition-opacity bg-popover/95 rounded">
                                <span className="text-[10px] text-muted-foreground pointer-events-none select-none">
                                  default
                                </span>
                                {onOpenAgentSettings && (
                                  <button
                                    onMouseDown={(e) => e.preventDefault()}
                                    onClick={(e) => {
                                      e.stopPropagation()
                                      close()
                                      onOpenAgentSettings("droid", "settings.json")
                                    }}
                                    className="p-0.5 rounded text-muted-foreground hover:text-foreground hover:bg-accent transition-colors pointer-events-auto"
                                    title="Edit Droid defaults"
                                  >
                                    <Settings className="w-3 h-3" />
                                  </button>
                                )}
                              </div>
                            )}
                          </div>
                        )
                      })}
                    </div>
                  )}
                </>
              ) : (
                <div className="p-3 text-xs text-muted-foreground whitespace-nowrap">Uses Claude Code local settings.</div>
              )}
            </div>
          </div>
        </>
      )}
    </div>
  )
}
