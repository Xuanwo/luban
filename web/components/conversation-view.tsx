"use client"

import type React from "react"
import { useState, useCallback } from "react"
import Image from "next/image"

import { Brain, Check, Clock, Copy, FileCode, FileText, Loader2, Wrench } from "lucide-react"

import { cn } from "@/lib/utils"
import type { Message } from "@/lib/conversation-ui"
import type { AttachmentRef } from "@/lib/luban-api"
import { Markdown } from "@/components/markdown"
import { ActivityStream } from "@/components/activity-stream"
import { attachmentHref } from "@/lib/attachment-href"

async function copyToClipboard(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text)
    return true
  } catch {
    const el = document.createElement("textarea")
    el.value = text
    el.style.position = "fixed"
    el.style.opacity = "0"
    document.body.appendChild(el)
    el.focus()
    el.select()
    const success = document.execCommand("copy")
    document.body.removeChild(el)
    return success
  }
}

function CopyButton({
  text,
  className,
  "data-testid": testId,
}: {
  text: string
  className?: string
  "data-testid"?: string
}): React.ReactElement {
  const [copied, setCopied] = useState(false)

  const handleCopy = useCallback(async () => {
    const success = await copyToClipboard(text)
    if (success) {
      setCopied(true)
      setTimeout(() => setCopied(false), 1500)
    }
  }, [text])

  return (
    <button
      type="button"
      data-testid={testId}
      className={cn(
        "transition-opacity hover:text-foreground p-1 -m-1 text-muted-foreground/70",
        className
      )}
      onClick={() => void handleCopy()}
      aria-label={copied ? "Copied" : "Copy message"}
    >
      {copied ? (
        <Check className="w-3 h-3 text-green-500" />
      ) : (
        <Copy className="w-3 h-3" />
      )}
    </button>
  )
}

export function ConversationMessage({
  message,
  workspaceId,
  renderAgentRunningCard,
}: {
  message: Message
  workspaceId?: number
  renderAgentRunningCard?: (message: Message) => React.ReactNode
}): React.ReactElement | null {
  if (message.type === "event") {
    return (
      <div
        className="text-[11px] text-muted-foreground/80"
        data-testid="conversation-event"
      >
        {message.status === "running" && (
          <Loader2
            data-testid="event-running-icon"
            className="inline-block w-3 h-3 animate-spin mr-1 align-[-2px]"
          />
        )}
        {message.eventSource === "agent" ? "Agent: " : message.eventSource === "system" ? "System: " : ""}
        {message.content}
      </div>
    )
  }

  return (
    <div className="group">
      {message.type === "assistant" ? (
        <div className="space-y-1" data-testid="assistant-message">
          {message.activities &&
            (renderAgentRunningCard?.(message) ?? (
              <ActivityStream
                activities={message.activities}
                isStreaming={message.isStreaming}
                isCancelled={message.isCancelled}
              />
            ))}

          {message.content && message.content.length > 0 && (
            <div className="luban-font-chat">
              <Markdown content={message.content} enableMermaid />
            </div>
          )}

          {message.codeReferences && message.codeReferences.length > 0 && (
            <div className="mt-3 flex flex-wrap gap-1.5">
              {message.codeReferences.map((ref, idx) => (
                <button
                  key={idx}
                  className="inline-flex items-center gap-1.5 px-2 py-1 bg-muted/50 hover:bg-primary/10 hover:text-primary rounded text-xs font-mono text-muted-foreground transition-all"
                >
                  <FileCode className="w-3 h-3" />
                  {ref.file}:{ref.line}
                </button>
              ))}
            </div>
          )}

          {message.metadata && !message.isStreaming && (
            <div className="flex items-center gap-3 pt-2 text-[11px] text-muted-foreground/70">
              {message.metadata.toolCalls && (
                <span className="flex items-center gap-1">
                  <Wrench className="w-3 h-3" />
                  {message.metadata.toolCalls}
                </span>
              )}
              {message.metadata.thinkingSteps && (
                <span className="flex items-center gap-1">
                  <Brain className="w-3 h-3" />
                  {message.metadata.thinkingSteps}
                </span>
              )}
              {message.metadata.duration && (
                <span className="flex items-center gap-1">
                  <Clock className="w-3 h-3" />
                  {message.metadata.duration}
                </span>
              )}
              <CopyButton
                text={message.content}
                className="ml-auto opacity-0 group-hover:opacity-100"
              />
            </div>
          )}
        </div>
      ) : (
        <div className="flex flex-col items-end">
          <div
            data-testid="user-message-bubble"
            className="relative max-w-[85%] border border-border rounded-lg px-3 py-2.5 bg-muted/30 luban-font-chat"
          >
            {message.attachments && message.attachments.length > 0 && (
              <div className="mb-2 flex flex-wrap gap-2">
                {message.attachments.map((attachment) => {
                  const href =
                    workspaceId != null ? attachmentHref({ workspaceId, attachment }) : null
                  return (
                    <a
                      key={`${attachment.kind}:${attachment.id}`}
                      data-testid="user-message-attachment"
                      href={href ?? undefined}
                      target={href ? "_blank" : undefined}
                      rel={href ? "noreferrer" : undefined}
                      className="group/att block w-20"
                    >
                      <div className="w-20 h-20 rounded-lg overflow-hidden border border-border/50 hover:border-border transition-colors bg-muted/40 flex items-center justify-center">
                        {attachment.kind === "image" && href ? (
                          <Image
                            src={href}
                            alt={attachment.name}
                            width={80}
                            height={80}
                            className="w-full h-full object-cover"
                            unoptimized
                          />
                        ) : (
                          <div className="flex flex-col items-center gap-1.5 px-2">
                            {attachment.extension.toLowerCase() === "json" ? (
                              <FileCode className="w-6 h-6 text-base09" />
                            ) : (
                              <FileText className="w-6 h-6 text-muted-foreground" />
                            )}
                            <span className="text-[9px] text-muted-foreground uppercase font-medium tracking-wide truncate w-full text-center">
                              {attachment.extension}
                            </span>
                          </div>
                        )}
                      </div>
                      <div className="mt-1 text-[10px] text-muted-foreground truncate">{attachment.name}</div>
                    </a>
                  )
                })}
              </div>
            )}
            <div className="text-[13px] text-foreground space-y-1 break-words overflow-hidden">
              {message.content.split("\n").map((line, idx) => (
                <p key={idx} className="flex items-start gap-2 min-w-0">
                  {line.startsWith("•") ? (
                    <>
                      <span className="text-muted-foreground mt-0.5 flex-shrink-0">•</span>
                      <span className="flex-1 min-w-0 break-words">{line.slice(2)}</span>
                    </>
                  ) : (
                    <span className="flex-1 min-w-0 break-words">{line}</span>
                  )}
                </p>
              ))}
            </div>
          </div>
          {message.content && message.content.trim().length > 0 && (
            <div className="flex justify-end pt-1">
              <CopyButton
                text={message.content}
                data-testid="user-message-copy"
                className="opacity-0 group-hover:opacity-100"
              />
            </div>
          )}
        </div>
      )}
    </div>
  )
}

export function ConversationView({
  messages,
  emptyState,
  className,
  workspaceId,
  renderAgentRunningCard,
}: {
  messages: Message[]
  emptyState?: React.ReactNode
  className?: string
  workspaceId?: number
  renderAgentRunningCard?: (message: Message) => React.ReactNode
}): React.ReactElement | null {
  if (messages.length === 0) {
    return emptyState ? <>{emptyState}</> : null
  }

  return (
    <div className={cn("space-y-4", className)}>
      {messages.map((message) => (
        <ConversationMessage
          key={message.id}
          message={message}
          workspaceId={workspaceId}
          renderAgentRunningCard={renderAgentRunningCard}
        />
      ))}
    </div>
  )
}
