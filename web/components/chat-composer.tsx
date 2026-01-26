"use client"

import type React from "react"

import { Send } from "lucide-react"

import { AgentSelector, type AgentRunnerOverride, type AmpModeOverride } from "@/components/shared/agent-selector"
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
  runnerOverride,
  ampModeOverride,
  onChangeRunnerOverride,
  onChangeAmpModeOverride,
  onSend,
  canSend,
  codexEnabled = true,
  ampEnabled = true,
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
  runnerOverride: AgentRunnerOverride
  ampModeOverride: AmpModeOverride
  onChangeRunnerOverride: (runner: AgentRunnerOverride) => void
  onChangeAmpModeOverride: (mode: AmpModeOverride) => void
  onSend: () => void
  canSend: boolean
  codexEnabled?: boolean
  ampEnabled?: boolean
}) {
  return (
    <div className="px-4 pb-4">
      <div className="max-w-3xl mx-auto">
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
          placeholder="Let's chart the cosmos of ideas..."
          disabled={disabled}
          agentSelector={
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
              runnerOverride={runnerOverride}
              ampModeOverride={ampModeOverride}
              onChangeRunnerOverride={onChangeRunnerOverride}
              onChangeAmpModeOverride={onChangeAmpModeOverride}
              codexEnabled={codexEnabled}
              ampEnabled={ampEnabled}
            />
          }
          primaryAction={{
            onClick: onSend,
            disabled: !canSend,
            ariaLabel: "Send message",
            icon: <Send className="w-3.5 h-3.5" />,
            testId: "chat-send",
          }}
          testIds={{
            textInput: "chat-input",
            attachInput: "chat-attach-input",
            attachButton: "chat-attach",
            attachmentTile: "chat-attachment-tile",
          }}
        />
      </div>
    </div>
  )
}
