"use client"

import type React from "react"

import { Send } from "lucide-react"

import { AgentSelector, type AmpMode } from "@/components/shared/agent-selector"
import { MessageEditor, type ComposerAttachment } from "@/components/shared/message-editor"
import type { AgentRunnerKind, AttachmentRef, CodexCustomPromptSnapshot, ThinkingEffort } from "@/lib/luban-api"

export function ChatComposer({
  value,
  onChange,
  attachments,
  onRemoveAttachment,
  onFileSelect,
  onPaste,
  onAddAttachmentRef,
  workspaceId,
  commands,
  messageHistory,
  onCommand,
  placeholder,
  attachmentsEnabled = true,
  agentSelectorEnabled = true,
  disabled,
  agentModelId,
  agentThinkingEffort,
  defaultModelId,
  defaultThinkingEffort,
  defaultAmpMode,
  onOpenAgentSettings,
  onChangeModelId,
  onChangeThinkingEffort,
  defaultRunner,
  runner,
  ampMode,
  onChangeRunner,
  onChangeAmpMode,
  onSend,
  secondaryAction,
  canSend,
  codexEnabled = true,
  ampEnabled = true,
  claudeEnabled = true,
  droidEnabled = true,
  runnerDefaultModels,
  compact = false,
}: {
  value: string
  onChange: (value: string) => void
  attachments: ComposerAttachment[]
  onRemoveAttachment: (id: string) => void
  onFileSelect: (files: FileList | null) => void
  onPaste: (e: React.ClipboardEvent) => void
  onAddAttachmentRef: (attachment: AttachmentRef) => void
  workspaceId: number | null
  commands: CodexCustomPromptSnapshot[]
  messageHistory: string[]
  onCommand: (commandId: string) => void
  placeholder?: string
  attachmentsEnabled?: boolean
  agentSelectorEnabled?: boolean
  disabled: boolean
  agentModelId: string | null | undefined
  agentThinkingEffort: ThinkingEffort | null | undefined
  defaultModelId: string | null
  defaultThinkingEffort: ThinkingEffort | null
  defaultAmpMode: string | null
  onOpenAgentSettings: (agentId: string, agentFilePath?: string) => void
  onChangeModelId: (modelId: string) => void
  onChangeThinkingEffort: (effort: ThinkingEffort) => void
  defaultRunner: AgentRunnerKind | null
  runner: AgentRunnerKind | null | undefined
  ampMode: string | null | undefined
  onChangeRunner: (runner: AgentRunnerKind) => void
  onChangeAmpMode: (mode: AmpMode) => void
  onSend: () => void
  secondaryAction?: {
    onClick: () => void
    ariaLabel: string
    icon: React.ReactNode
    testId?: string
  }
  canSend: boolean
  codexEnabled?: boolean
  ampEnabled?: boolean
  claudeEnabled?: boolean
  droidEnabled?: boolean
  runnerDefaultModels?: Record<string, string> | null
  /** When true, removes padding and max-width constraints for embedding in cards */
  compact?: boolean
}) {
  // Card-style for compact mode (matches Linear activity card styling)
  const compactStyle = compact ? {
    border: '1px solid #e8e8e8',
    borderRadius: '8px',
    boxShadow: 'rgba(0,0,0,0.022) 0px 3px 6px -2px, rgba(0,0,0,0.044) 0px 1px 1px 0px',
  } : undefined

  const agentSelectorNode = agentSelectorEnabled ? (
    <AgentSelector
      testId="agent-selector"
      dropdownPosition="top"
      disabled={disabled}
      modelId={agentModelId}
      thinkingEffort={agentThinkingEffort}
      defaultModelId={defaultModelId}
      defaultThinkingEffort={defaultThinkingEffort}
      defaultAmpMode={defaultAmpMode}
      onOpenAgentSettings={onOpenAgentSettings}
      onChangeModelId={onChangeModelId}
      onChangeThinkingEffort={onChangeThinkingEffort}
      defaultRunner={defaultRunner}
      runner={runner}
      ampMode={ampMode}
      onChangeRunner={onChangeRunner}
      onChangeAmpMode={onChangeAmpMode}
      codexEnabled={codexEnabled}
      ampEnabled={ampEnabled}
      claudeEnabled={claudeEnabled}
      droidEnabled={droidEnabled}
      runnerDefaultModels={runnerDefaultModels}
    />
  ) : null

  const content = (
        <MessageEditor
          value={value}
          onChange={onChange}
          attachments={attachments}
          onRemoveAttachment={onRemoveAttachment}
          onFileSelect={onFileSelect}
          onPaste={onPaste}
          onAddAttachmentRef={onAddAttachmentRef}
          workspaceId={workspaceId}
          commands={commands}
          messageHistory={messageHistory}
          onCommand={onCommand}
          placeholder={placeholder ?? "Let's chart the cosmos of ideas..."}
          attachmentsEnabled={attachmentsEnabled}
          disabled={disabled}
          agentSelector={
            agentSelectorNode
          }
          primaryAction={{
            onClick: onSend,
            disabled: !canSend,
            ariaLabel: "Send message",
            icon: <Send className="w-3.5 h-3.5" />,
            testId: "chat-send",
          }}
          secondaryAction={secondaryAction}
          testIds={{
            textInput: "chat-input",
            attachInput: "chat-attach-input",
            attachButton: "chat-attach",
            attachmentTile: "chat-attachment-tile",
          }}
          style={compactStyle}
        />
  )

  if (compact) {
    return content
  }

  return (
    <div className="px-4 pb-4">
      <div className="max-w-3xl mx-auto">
        {content}
      </div>
    </div>
  )
}
