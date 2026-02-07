"use client"

import type React from "react"
import { useEffect, useMemo, useState, useCallback, useRef } from "react"
import Image from "next/image"
import {
  Brain,
  Check,
  CheckCircle2,
  ChevronsUpDown,
  Clock,
  Copy,
  FileCode,
  FileText,
  Loader2,
  MessageSquareText,
  Pause,
  Pencil,
  Terminal,
  Wrench,
  Eye,
  X,
} from "lucide-react"

import { cn } from "@/lib/utils"
import { agentRunnerLabel } from "@/lib/conversation-ui"
import type { Message, ActivityEvent } from "@/lib/conversation-ui"
import { Markdown } from "@/components/markdown"
import { AnsiOutput } from "@/components/shared/ansi-output"
import { UnifiedProviderLogo } from "@/components/shared/unified-provider-logo"
import { extractTurnDurationLabel, useActivityTiming } from "@/lib/activity-timing"
import { attachmentHref } from "@/lib/attachment-href"
import { WindowedList, type WindowedListItem } from "@/components/windowed-list"
import { PtyTerminalSession } from "@/components/pty-terminal"
import { useLuban } from "@/lib/luban-context"

const AMP_MARK_URL = "/logos/amp.svg"

function AgentRunnerIcon({
  runner,
  className,
}: {
  runner: Message["agentRunner"]
  className?: string
}): React.ReactElement {
  if (runner === "amp") {
    return (
      <Image
        data-agent-runner-icon="amp"
        src={AMP_MARK_URL}
        alt=""
        aria-hidden="true"
        className={className}
        width={14}
        height={14}
      />
    )
  }

  if (runner === "claude") {
    return <UnifiedProviderLogo providerId="anthropic" className={className} />
  }

  if (runner === "droid") {
    return <UnifiedProviderLogo providerId="factory" className={className} />
  }

  return <UnifiedProviderLogo providerId="openai" className={className} />
}

/**
 * Linear Design System (extracted from Linear app via agent-browser):
 * 
 * Layout:
 * - Content area max-width: 686px
 * - Content area margin: 0 60px (creates side padding)
 * - Content starts at left: 305px from window edge (after 244px sidebar + 61px margin)
 * 
 * Colors:
 * - Primary text: #1a1a1a (lch 9.723)
 * - Secondary text: #2f2f2f (lch 19.446)
 * - Muted text: #5f5f5f (lch 38.893)
 * - Background: #fdfdfd (lch 99)
 * 
 * Comment Container:
 * - padding: 16px
 * - display: flex, flex-direction: column
 * 
 * Comment Header Row:
 * - Avatar: 18px (with 2px vertical padding for 14px inner), border-radius: 50%
 * - Gap avatar to username: 11px
 * - Username: 12px, font-weight 500, muted color
 * - Timestamp: 12px, font-weight 450, muted color
 * 
 * Comment Content:
 * - font-size: 15px, font-weight: 450, line-height: 24px
 * - color: lch(19.446) ≈ #2f2f2f
 * - Aligned with avatar left edge
 */

const COLORS = {
  textPrimary: '#1b1b1b',
  textSecondary: '#2f2f2f',
  textMuted: '#5b5b5d',
  background: '#fdfdfd',
  white: '#ffffff',
  border: '#e8e8e8',
  timeline: '#c8c8c8',
  accent: '#5e6ad2',
  warning: '#f2994a',
}

const ACTIVITY_AXIS_X_PX = 21
const ACTIVITY_ICON_SIZE_PX = 14
const ACTIVITY_ICON_RADIUS_PX = ACTIVITY_ICON_SIZE_PX / 2
const ACTIVITY_ICON_SLOT_HEIGHT_PX = 16.8
const ACTIVITY_TIMELINE_LEFT_PX = ACTIVITY_AXIS_X_PX - 0.5

/**
 * Linear Activity Layout (from linear-activity-spec.md):
 * 
 * - Comment cards have: border 1px #e8e8e8, border-radius 8px, white background, subtle shadow
 * - Simple events (status changes, etc.) have NO card - inline text only
 * - Timeline line connects simple events vertically (1px, #c8c8c8), breaks at comment cards
 * - Avatar: 20x20px circular (border-radius: 50%)
 * - Simple events: icon 14x14px circular, text 12px muted
 * - Comment card padding: 12px 16px
 */

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
}: {
  text: string
  className?: string
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
      className={cn(
        "transition-opacity hover:opacity-70 p-1 -m-1",
        className
      )}
      onClick={() => void handleCopy()}
      aria-label={copied ? "Copied" : "Copy"}
      style={{ color: COLORS.textMuted }}
    >
      {copied ? (
        <Check className="w-3 h-3" style={{ color: COLORS.accent }} />
      ) : (
        <Copy className="w-3 h-3" />
      )}
    </button>
  )
}

function formatRelativeTime(timestamp?: string): string {
  if (!timestamp) return ""
  const date = new Date(timestamp)
  const now = new Date()
  const diffMs = now.getTime() - date.getTime()
  const diffMins = Math.floor(diffMs / 60000)
  const diffHours = Math.floor(diffMins / 60)
  const diffDays = Math.floor(diffHours / 24)
  const diffMonths = Math.floor(diffDays / 30)

  if (diffMins < 1) return "now"
  if (diffMins < 60) return `${diffMins}m ago`
  if (diffHours < 24) return `${diffHours}h ago`
  if (diffDays < 30) return `${diffDays}d ago`
  return `${diffMonths}mo ago`
}

interface ActivityEventItemProps {
  event: ActivityEvent
  isExpanded: boolean
  onToggle: () => void
  duration: string | null
}

function ActivityEventItem({ event, isExpanded, onToggle, duration }: ActivityEventItemProps) {
  const detail = typeof event.detail === "string" ? event.detail : ""
  const isAssistantMessage = event.type === "assistant_message"
  const hasExpandableDetail = event.type === "bash" || detail.trim().length > 0
  const canToggle = hasExpandableDetail && !isAssistantMessage
  const shouldShowDetail = hasExpandableDetail && isExpanded
  const hasDuration = !isAssistantMessage && typeof duration === "string" && duration.trim().length > 0
  const subagent = event.subagent ?? null
  const subagentStatusLabel = (() => {
    if (!subagent) return ""
    if (subagent.status === "running") return "RUNNING"
    if (subagent.status === "done") return "DONE"
    if (subagent.status === "blocked") return "BLOCKED"
    if (subagent.status === "failed") return "FAILED"
    if (subagent.status === "canceled") return "CANCELED"
    return "UNKNOWN"
  })()
  const subagentStatusColor = (() => {
    if (!subagent) return COLORS.textMuted
    if (subagent.status === "running") return COLORS.accent
    if (subagent.status === "done") return "#15803d"
    if (subagent.status === "blocked") return COLORS.warning
    if (subagent.status === "failed" || subagent.status === "canceled") return "#dc2626"
    return COLORS.textMuted
  })()

  const icon = (() => {
    switch (event.type) {
      case "thinking":
        return <Brain className="w-3.5 h-3.5" />
      case "tool_call":
        return <Wrench className="w-3.5 h-3.5" />
      case "file_edit":
        return <Pencil className="w-3.5 h-3.5" />
      case "bash":
        return <Terminal className="w-3.5 h-3.5" />
      case "search":
        return <Eye className="w-3.5 h-3.5" />
      case "complete":
        return <CheckCircle2 className="w-3.5 h-3.5" />
      case "assistant_message":
        return <MessageSquareText className="w-3.5 h-3.5" />
      default:
        return <Wrench className="w-3.5 h-3.5" />
    }
  })()

  if (isAssistantMessage) {
    const assistantIcon = event.status === "running" ? (
      <Loader2 data-testid="event-running-icon" className="w-3.5 h-3.5 animate-spin" />
    ) : (
      icon
    )

    return (
      <div className="group/event py-1">
        <div className="flex items-start gap-2">
          <div
            data-testid="activity-event-icon"
            className="flex items-center justify-center flex-shrink-0"
            style={{ width: "14px", height: "16.8px", marginLeft: "3px", marginTop: "2px" }}
          >
            {assistantIcon}
          </div>
          <div
            className="flex-1 py-1.5 mb-1 rounded luban-font-chat"
            style={{
              paddingLeft: "8px",
              paddingRight: "8px",
              fontSize: "14px",
              lineHeight: "22px",
              color: COLORS.textPrimary,
              backgroundColor: "rgba(0,0,0,0.02)",
            }}
          >
            <Markdown content={detail} enableMermaid />
          </div>
        </div>
      </div>
    )
  }

  return (
    <div className="group/event">
      <button
        onClick={() => canToggle && onToggle()}
        className={cn(
          "w-full flex items-center gap-2 py-1 rounded transition-colors",
          canToggle ? "hover:bg-black/[0.03] cursor-pointer" : "cursor-default"
        )}
        style={{ 
          fontSize: '12px',
          fontWeight: 450,
          lineHeight: '16.8px',
          color: event.status === "running" ? COLORS.accent : COLORS.textMuted
        }}
      >
        <div
          data-testid="activity-event-icon"
          className="flex items-center justify-center flex-shrink-0"
          style={{ width: '14px', height: '16.8px', marginLeft: '3px' }}
        >
          {event.status === "running" ? (
            <Loader2 data-testid="event-running-icon" className="w-3.5 h-3.5 animate-spin" />
          ) : (
            icon
          )}
        </div>
        <span data-testid="activity-event-title" className="flex-1 text-left truncate">
          {event.title}
        </span>
        <div data-testid="activity-event-trailing" className="flex items-center gap-1 flex-shrink-0">
          {subagent && (
            <>
              <span
                data-testid="activity-subagent-pill"
                className="px-1.5 py-0.5 rounded"
                style={{
                  fontSize: "10px",
                  fontWeight: 600,
                  letterSpacing: "0.02em",
                  color: COLORS.textMuted,
                  backgroundColor: "rgba(0,0,0,0.04)",
                }}
              >
                SUBTASK
              </span>
              <span
                data-testid="activity-subagent-status"
                className="px-1.5 py-0.5 rounded"
                style={{
                  fontSize: "10px",
                  fontWeight: 600,
                  letterSpacing: "0.02em",
                  color: subagentStatusColor,
                  backgroundColor: `${subagentStatusColor}22`,
                }}
              >
                {subagentStatusLabel}
              </span>
            </>
          )}
          {hasDuration && (
            <span
              data-testid="activity-event-duration"
              className="font-mono tabular-nums text-right flex-shrink-0"
              style={{ fontSize: "10px", color: COLORS.textMuted }}
            >
              {duration}
            </span>
          )}
        </div>
      </button>
      {shouldShowDetail && (
        <div
          className="py-1.5 mb-1 rounded font-mono"
          style={{
            marginLeft: '25px',
            paddingLeft: '8px',
            paddingRight: '8px',
            fontSize: "11px",
            color: COLORS.textMuted,
            backgroundColor: 'rgba(0,0,0,0.02)',
          }}
        >
          <AnsiOutput text={detail} />
        </div>
      )}
    </div>
  )
}

interface SystemEventItemProps {
  message: Message
  /** Who triggered this event - defaults to "You" for user events */
  actor?: {
    name: string
    initial: string
    color: string
  }
}

function SystemEventItem({ message, actor }: SystemEventItemProps) {
  const defaultActor = (() => {
    if (message.eventSource === "agent") {
      return { name: agentRunnerLabel(message.agentRunner), initial: "A", color: COLORS.textPrimary }
    }
    if (message.eventSource === "system") {
      return { name: "Luban", initial: "L", color: COLORS.textMuted }
    }
    return { name: "You", initial: "U", color: COLORS.accent }
  })()
  const eventActor = actor || defaultActor
  const isAgentEvent = message.eventSource === "agent"
  const showLubanLogo = !actor && message.eventSource === "system"

  return (
    <div 
      className="flex items-start"
      style={{ padding: '1px 0' }}
      data-testid="activity-event"
    >
      <div
        className="flex items-center justify-center flex-shrink-0 relative z-10"
        style={{ 
          width: ACTIVITY_ICON_SIZE_PX,
          height: ACTIVITY_ICON_SLOT_HEIGHT_PX,
          marginLeft: ACTIVITY_AXIS_X_PX - ACTIVITY_ICON_RADIUS_PX,
          marginRight: '11px',
          backgroundColor: COLORS.background
        }}
      >
        <div
          data-testid="event-avatar"
          className="flex items-center justify-center"
          style={{ 
            width: ACTIVITY_ICON_SIZE_PX,
            height: ACTIVITY_ICON_SIZE_PX,
            borderRadius: '50%',
            backgroundColor: showLubanLogo || isAgentEvent ? COLORS.white : eventActor.color,
            border: showLubanLogo || isAgentEvent ? `1px solid ${COLORS.border}` : undefined,
            color: showLubanLogo || isAgentEvent ? COLORS.textPrimary : COLORS.white,
            fontSize: '7px',
            fontWeight: 500
          }}
        >
          {showLubanLogo ? (
            <Image src="/icon-light-32x32.png" alt="Luban" width={10} height={10} unoptimized />
          ) : isAgentEvent ? (
            <AgentRunnerIcon runner={message.agentRunner} className="w-2.5 h-2.5" />
          ) : (
            eventActor.initial
          )}
        </div>
      </div>
      
      {/* Event text - Linear style: 12px, muted colors, inline */}
      <span 
        data-testid="event-text"
        className="flex items-center flex-wrap"
        style={{ fontSize: '12px', lineHeight: '16.8px', color: COLORS.textMuted }}
      >
        <b data-testid="activity-simple-author" style={{ fontWeight: 500 }}>
          {eventActor.name}
        </b>
        <span style={{ marginLeft: '4px' }}>{message.content}</span>
        <span style={{ margin: '0 4px' }}>·</span>
        <span className="inline-flex items-center gap-1">
          {message.status === "running" ? (
            <Loader2
              data-testid="event-running-icon"
              className="w-3 h-3 animate-spin flex-shrink-0"
              style={{ color: COLORS.textMuted }}
            />
          ) : (
            <span data-testid="event-timestamp">{formatRelativeTime(message.timestamp)}</span>
          )}
        </span>
      </span>
    </div>
  )
}

interface CollapsedEventsGroupProps {
  events: Message[]
  onExpand: () => void
}

function CollapsedEventsGroup({ events, onExpand }: CollapsedEventsGroupProps) {
  const tail = events.slice(Math.max(0, events.length - 3))
  const hiddenCount = Math.max(0, events.length - tail.length)
  const summaryParts = tail.map((e) => e.content)
  const summary = summaryParts.join(", ") + (hiddenCount > 0 ? "..." : "")
  
  return (
      <div className="flex flex-col gap-2">
        <div className="flex items-center" style={{ padding: '1px 0' }}>
          <div
            className="flex items-center justify-center flex-shrink-0 relative z-10"
            style={{ 
              width: ACTIVITY_ICON_SIZE_PX,
              marginLeft: ACTIVITY_AXIS_X_PX - ACTIVITY_ICON_RADIUS_PX,
              marginRight: '11px',
              backgroundColor: COLORS.background
            }}
          >
            <ChevronsUpDown 
              className="w-3.5 h-3.5" 
            style={{ color: COLORS.textMuted }}
          />
        </div>
        
        <button
          onClick={onExpand}
          className="flex-1 min-w-0 hover:underline cursor-pointer text-left truncate"
          style={{ fontSize: '12px', lineHeight: '16.8px', color: COLORS.textMuted }}
        >
          Show {hiddenCount} earlier events: {summary}
        </button>
      </div>

      <div className="flex flex-col gap-2">
        {tail.map((message, index) => {
          const hasNextEvent = index < tail.length - 1
          return (
            <div key={message.id} className="relative">
              {hasNextEvent && (
                <div 
                  className="absolute"
                  style={{
                    left: `${ACTIVITY_TIMELINE_LEFT_PX}px`,
                    top: '20px',
                    bottom: '-8px',
                    width: '1px',
                    backgroundColor: COLORS.timeline
                  }}
                />
              )}
              <SystemEventItem message={message} />
            </div>
          )
        })}
      </div>
    </div>
  )
}

interface UserActivityEventProps {
  message: Message
  workspaceId?: number
}

function UserActivityEvent({ message, workspaceId }: UserActivityEventProps) {
  return (
    <div 
      className="group/activity"
      style={{
        border: `1px solid ${COLORS.border}`,
        borderRadius: '8px',
        backgroundColor: COLORS.white,
        boxShadow: 'rgba(0,0,0,0.022) 0px 3px 6px -2px, rgba(0,0,0,0.044) 0px 1px 1px 0px',
        padding: '12px 16px',
        marginLeft: '-6px',  /* Extend card border outward for avatar alignment */
        marginRight: '-6px'
      }}
    >
      {/* Header row: Avatar + Username + Timestamp */}
      <div className="flex items-center gap-2 mb-2">
        {/* Avatar - 20x20 circular (Linear style) */}
        <div
          className="flex items-center justify-center flex-shrink-0"
          style={{ width: "20px", height: "20px" }}
        >
          <div
            data-testid="activity-card-avatar-inner"
            className="flex items-center justify-center text-white"
            style={{
              width: "16px",
              height: "16px",
              borderRadius: "50%",
              backgroundColor: COLORS.accent,
              fontSize: "8px",
              fontWeight: 500,
              lineHeight: "16px",
            }}
          >
            U
          </div>
        </div>
        <span data-testid="activity-card-author" style={{ fontSize: '13px', fontWeight: 500, color: COLORS.textPrimary }}>
          You
        </span>
        <span style={{ fontSize: '14px', fontWeight: 400, color: COLORS.textMuted }}>
          {formatRelativeTime(message.timestamp)}
        </span>
        <CopyButton
          text={message.content}
          className="ml-auto opacity-0 group-hover/activity:opacity-100 transition-opacity"
        />
      </div>

      {message.attachments && message.attachments.length > 0 && (
        <div className="mb-2 flex flex-wrap gap-2">
          {message.attachments.map((attachment) => {
            const href = workspaceId != null ? attachmentHref({ workspaceId, attachment }) : null
            const ext = attachment.extension.toLowerCase()
            const isJson = ext === "json"
            return (
              <a
                key={`${attachment.kind}:${attachment.id}`}
                data-testid="activity-user-attachment"
                href={href ?? undefined}
                target={href ? "_blank" : undefined}
                rel={href ? "noreferrer" : undefined}
                className="group/att block w-20"
              >
                <div className="w-20 h-20 rounded-lg overflow-hidden border border-border/50 hover:border-border transition-colors bg-muted/40 flex items-center justify-center">
                  {attachment.kind === "image" && href ? (
                    // eslint-disable-next-line @next/next/no-img-element
                    <img src={href} alt={attachment.name} className="w-full h-full object-cover" />
                  ) : (
                    <div className="flex flex-col items-center gap-1.5 px-2">
                      {isJson ? (
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
      
      {/* Message content */}
      <div
        data-testid="activity-user-message-content"
        style={{ fontSize: '13px', fontWeight: 400, lineHeight: '1.625', color: COLORS.textPrimary }}
      >
        {message.content.split("\n").map((line, idx) => (
          <p key={idx} className="min-h-[1.625em]">
            {line || "\u00A0"}
          </p>
        ))}
      </div>
    </div>
  )
}

function TerminalCommandCardEvent({
  message,
  workspaceId,
  taskId,
}: {
  message: Message
  workspaceId?: number
  taskId?: number
}) {
  const tc = message.terminalCommand
  if (!tc || workspaceId == null || taskId == null) {
    return (
      <div
        className="group/activity"
        data-testid="activity-terminal-command-card"
        style={{ fontSize: "12px", color: COLORS.textMuted }}
      >
        Terminal output unavailable
      </div>
    )
  }

  const isRunning = tc.status === "running"
  const hasOutput = (tc.outputByteLen ?? 0) > 0

  return (
    <div
      className="group/activity"
      data-testid="activity-terminal-command-card"
      style={{
        border: `1px solid ${COLORS.border}`,
        borderRadius: "8px",
        backgroundColor: COLORS.white,
        boxShadow: "rgba(0,0,0,0.022) 0px 3px 6px -2px, rgba(0,0,0,0.044) 0px 1px 1px 0px",
        padding: "12px 16px",
        marginLeft: "-6px",
        marginRight: "-6px",
      }}
    >
      <div className="flex items-center gap-2 mb-2">
        <div className="flex items-center justify-center flex-shrink-0" style={{ width: "20px", height: "20px" }}>
          <div
            className="flex items-center justify-center"
            style={{
              width: "16px",
              height: "16px",
              borderRadius: "50%",
              backgroundColor: COLORS.accent,
              color: COLORS.white,
            }}
          >
            <Terminal className="w-3 h-3" />
          </div>
        </div>
        <span style={{ fontSize: "13px", fontWeight: 500, color: COLORS.textPrimary }}>You</span>
        <span style={{ fontSize: "14px", fontWeight: 400, color: COLORS.textMuted }}>
          {isRunning ? "Running" : formatRelativeTime(message.timestamp)}
        </span>
        <CopyButton
          text={tc.command}
          className="ml-auto opacity-0 group-hover/activity:opacity-100 transition-opacity"
        />
      </div>

      <div className="mb-2" style={{ fontSize: "13px", fontWeight: 500, color: COLORS.textPrimary }}>
        <code data-testid="activity-terminal-command" className="font-mono text-[12px]">
          {tc.command}
        </code>
      </div>

      <div
        data-testid="activity-terminal-output"
        style={{
          height: "260px",
          border: `1px solid ${COLORS.border}`,
          borderRadius: "8px",
          overflow: "hidden",
          backgroundColor: "#fcfcfc",
        }}
      >
        <PtyTerminalSession
          workspaceId={workspaceId}
          threadId={taskId}
          reconnectToken={tc.reconnect}
          readOnly={!isRunning}
          initialBase64={!isRunning && hasOutput ? tc.outputBase64 ?? "" : ""}
          testId="activity-terminal-pty"
        />
      </div>
    </div>
  )
}

function taskStatusLabel(status: string): string {
  switch (status) {
    case "backlog":
      return "Backlog"
    case "todo":
      return "Todo"
    case "iterating":
    case "in_progress":
      return "Iterating"
    case "validating":
    case "in_review":
      return "Validating"
    case "done":
      return "Done"
    case "canceled":
      return "Canceled"
    default:
      return status
  }
}

function TaskStatusSuggestionCardEvent({
  message,
  workspaceId,
  taskId,
}: {
  message: Message
  workspaceId?: number
  taskId?: number
}) {
  const { setTaskStatus } = useLuban()
  const suggestion = message.taskStatusSuggestion
  const [isExpanded, setIsExpanded] = useState(false)

  if (!suggestion || workspaceId == null || taskId == null) {
    return null
  }

  const title = (() => {
    const trimmed = suggestion.title.trim()
    if (trimmed.length > 0) return trimmed
    return `Suggested moving from ${taskStatusLabel(suggestion.from)} to ${taskStatusLabel(suggestion.to)}`
  })()

  const explanation = suggestion.explanationMarkdown.trim()

  return (
    <div
      data-testid="task-status-suggestion-card"
      className="group/suggestion"
      style={{
        border: `1px solid ${COLORS.border}`,
        borderRadius: "8px",
        backgroundColor: COLORS.white,
        boxShadow: "rgba(0,0,0,0.022) 0px 3px 6px -2px, rgba(0,0,0,0.044) 0px 1px 1px 0px",
        padding: "12px 16px",
        marginLeft: "-6px",
        marginRight: "-6px",
      }}
    >
      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={() => setIsExpanded((prev) => !prev)}
          className="flex items-center gap-2 flex-1 min-w-0 text-left"
          style={{ color: COLORS.textMuted }}
          data-testid="task-status-suggestion-toggle"
        >
          <div className="flex items-center justify-center flex-shrink-0" style={{ width: "20px", height: "20px" }}>
            <div
              data-testid="task-status-suggestion-avatar-inner"
              className="flex items-center justify-center"
              style={{
                width: "16px",
                height: "16px",
                borderRadius: "50%",
                backgroundColor: COLORS.white,
                border: `1px solid ${COLORS.border}`,
                color: COLORS.textPrimary,
              }}
            >
              <Image src="/icon-light-32x32.png" alt="Luban" width={10} height={10} unoptimized />
            </div>
          </div>

          <span
            data-testid="activity-card-author"
            style={{ fontSize: "13px", fontWeight: 500, color: COLORS.textPrimary }}
          >
            Luban
          </span>
          <span
            className="truncate"
            style={{ fontSize: "13px", fontWeight: 400, color: COLORS.textMuted }}
          >
            {title}
          </span>
          <span style={{ fontSize: "14px", fontWeight: 400, color: COLORS.textMuted }}>
            {formatRelativeTime(message.timestamp)}
          </span>
          <ChevronsUpDown className="w-4 h-4 flex-shrink-0" />
        </button>

        <button
          type="button"
          data-testid="task-status-suggestion-apply"
          className="px-2 py-1 rounded transition-colors hover:bg-black/[0.05]"
          style={{ fontSize: "12px", fontWeight: 500, color: COLORS.textPrimary }}
          onClick={(e) => {
            e.preventDefault()
            e.stopPropagation()
            setTaskStatus(workspaceId, taskId, suggestion.to)
          }}
        >
          Apply
        </button>
      </div>

      {isExpanded && (
        <div className="mt-3" data-testid="task-status-suggestion-body">
          <div style={{ fontSize: "12px", color: COLORS.textMuted, marginBottom: "6px" }}>
            Suggested status: {taskStatusLabel(suggestion.to)}
          </div>
          {explanation.length > 0 ? (
            <div className="luban-font-chat" style={{ fontSize: "14px", lineHeight: "22px", color: COLORS.textPrimary }}>
              <Markdown content={explanation} enableMermaid />
            </div>
          ) : (
            <div style={{ fontSize: "13px", color: COLORS.textMuted }}>No explanation provided.</div>
          )}
        </div>
      )}
    </div>
  )
}

interface AgentActivityEventProps {
  message: Message
}

function AgentActivityEvent({ message }: AgentActivityEventProps) {
  const hasContent = message.content.length > 0
  const agentName = agentRunnerLabel(message.agentRunner)

  return (
    <div className="flex flex-col gap-2">
      {/* Agent message content as a card (only if there's content) */}
      {hasContent && (
        <div 
          className="group/activity"
          style={{
            border: `1px solid ${COLORS.border}`,
            borderRadius: '8px',
            backgroundColor: COLORS.white,
            boxShadow: 'rgba(0,0,0,0.022) 0px 3px 6px -2px, rgba(0,0,0,0.044) 0px 1px 1px 0px',
            padding: '12px 16px',
            marginLeft: '-6px',
            marginRight: '-6px'
          }}
        >
          {/* Header row: Avatar + Agent name + Timestamp + Duration */}
          <div className="flex items-center gap-2 mb-2">
            <div
              className="flex items-center justify-center flex-shrink-0"
              style={{ width: "20px", height: "20px" }}
            >
              <div
                data-testid="activity-card-avatar-inner"
                className="flex items-center justify-center"
                style={{
                  width: "16px",
                  height: "16px",
                  borderRadius: "50%",
                  backgroundColor: COLORS.white,
                  border: `1px solid ${COLORS.border}`,
                  color: COLORS.textPrimary,
                }}
              >
                <AgentRunnerIcon runner={message.agentRunner} className="w-3 h-3" />
              </div>
            </div>
            <span
              data-testid="activity-card-author"
              style={{ fontSize: '13px', fontWeight: 500, color: COLORS.textPrimary }}
            >
              {agentName}
            </span>
            <span style={{ fontSize: '14px', fontWeight: 400, color: COLORS.textMuted }}>
              {formatRelativeTime(message.timestamp)}
            </span>
            {message.metadata?.duration && (
              <span 
                className="flex items-center gap-1"
                style={{ fontSize: '14px', fontWeight: 400, color: COLORS.textMuted }}
              >
                <Clock className="w-3.5 h-3.5" />
                {message.metadata.duration}
              </span>
            )}
            <CopyButton
              text={message.content}
              className="ml-auto opacity-0 group-hover/activity:opacity-100 transition-opacity"
            />
          </div>

          {/* Agent message content */}
          <div 
            data-testid="activity-agent-message-content"
            className="luban-font-chat"
            style={{ fontSize: '15px', fontWeight: 400, lineHeight: '24px', color: COLORS.textPrimary }}
          >
            <Markdown content={message.content} enableMermaid />
          </div>

          {/* Code references */}
          {message.codeReferences && message.codeReferences.length > 0 && (
            <div className="flex flex-wrap gap-1.5 mt-2">
              {message.codeReferences.map((ref, idx) => (
                <button
                  key={idx}
                  className="inline-flex items-center gap-1.5 px-2 py-1 rounded font-mono transition-colors hover:bg-black/[0.05]"
                  style={{ 
                    fontSize: '12px',
                    backgroundColor: 'rgba(0,0,0,0.03)', 
                    color: COLORS.textMuted 
                  }}
                >
                  <FileCode className="w-3 h-3" />
                  {ref.file}:{ref.line}
                </button>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  )
}

function AgentTurnCardEvent({
  message,
  onCancel,
  isExpanded: controlledExpanded,
  onExpandedChange,
  expandedEvents: controlledExpandedEvents,
  onToggleEvent,
}: {
  message: Message
  onCancel?: () => void
  isExpanded?: boolean
  onExpandedChange?: (expanded: boolean) => void
  expandedEvents?: Set<string>
  onToggleEvent?: (eventId: string) => void
}) {
  const activities = message.activities ?? []
  const [uncontrolledExpanded, setUncontrolledExpanded] = useState(false)
  const [uncontrolledExpandedEvents, setUncontrolledExpandedEvents] = useState<Set<string>>(new Set())
  const isExpanded = controlledExpanded ?? uncontrolledExpanded
  const expandedEvents = controlledExpandedEvents ?? uncontrolledExpandedEvents
  const setExpanded = onExpandedChange ?? setUncontrolledExpanded
  const { durationLabel } = useActivityTiming(activities)

  const isRunning = message.turnStatus === "running"
  const isCancelled = message.turnStatus === "canceled"
  const isErrored = message.turnStatus === "error"

  const latestActivity = activities[activities.length - 1]
  const completedCount = activities.filter((a) => a.status === "done" && a.title !== "Turn canceled").length
  const turnDurationLabel = extractTurnDurationLabel(activities)
  const assistantMessageEvents = activities.filter((event) => event.type === "assistant_message")
  const latestAssistantMessage = assistantMessageEvents[assistantMessageEvents.length - 1]
  const collapsedMessageContent = (() => {
    const latestMessageText = (typeof latestAssistantMessage?.detail === "string" ? latestAssistantMessage.detail : "").trim()
    const fullMessageText = message.content.trim()
    if (isRunning) return latestMessageText.length > 0 ? latestMessageText : fullMessageText
    return fullMessageText.length > 0 ? fullMessageText : latestMessageText
  })()
  const hasCollapsedContent = !isExpanded && collapsedMessageContent.length > 0

  const toggleEvent =
    onToggleEvent ??
    ((eventId: string) => {
      setUncontrolledExpandedEvents((prev) => {
        const next = new Set(prev)
        if (next.has(eventId)) {
          next.delete(eventId)
        } else {
          next.add(eventId)
        }
        return next
      })
    })

  const summary = (() => {
    if (isRunning) return latestActivity?.title ?? "Processing"
    const suffix = turnDurationLabel ? ` in ${turnDurationLabel}` : ""
    if (isCancelled) return `Cancelled after ${completedCount} steps${suffix}`
    if (isErrored) return `Failed after ${completedCount} steps${suffix}`
    return `Completed ${completedCount} steps${suffix}`
  })()

  const agentName = agentRunnerLabel(message.agentRunner)
  const timestampLabel = formatRelativeTime(message.timestamp)

  return (
    <div
      data-testid="agent-turn-card"
      className="group/turn"
      style={{
        border: `1px solid ${COLORS.border}`,
        borderRadius: "8px",
        backgroundColor: COLORS.white,
        boxShadow: "rgba(0,0,0,0.022) 0px 3px 6px -2px, rgba(0,0,0,0.044) 0px 1px 1px 0px",
        padding: "12px 16px",
        marginLeft: "-6px",
        marginRight: "-6px",
      }}
    >
      <div className="flex items-center gap-2">
	        <button
	          data-testid="agent-turn-toggle"
	          type="button"
	          onClick={() => setExpanded(!isExpanded)}
	          className="flex items-center gap-2 flex-1 min-w-0 text-left"
	          style={{ color: COLORS.textMuted }}
	        >
          <div
            data-testid="agent-turn-avatar"
            className="flex items-center justify-center flex-shrink-0"
            style={{
              width: "20px",
              height: "20px",
            }}
          >
            <div
              data-testid="activity-card-avatar-inner"
              className="flex items-center justify-center"
              style={{
                width: "16px",
                height: "16px",
                borderRadius: "50%",
                backgroundColor: COLORS.white,
                border: `1px solid ${COLORS.border}`,
                color: COLORS.textPrimary,
              }}
            >
              <AgentRunnerIcon runner={message.agentRunner} className="w-3 h-3" />
            </div>
          </div>
          <span data-testid="activity-card-author" style={{ fontSize: "13px", fontWeight: 500, color: COLORS.textPrimary }}>
            {agentName}
          </span>
          {timestampLabel.length > 0 && (
            <span style={{ fontSize: "14px", fontWeight: 400, color: COLORS.textMuted }}>{timestampLabel}</span>
          )}
          <span
            className="flex-1 min-w-0 truncate"
            style={{ fontSize: "12px", fontWeight: 450, color: COLORS.textMuted }}
          >
            {summary}
          </span>
        </button>

        {hasCollapsedContent && (
          <CopyButton
            text={collapsedMessageContent}
            className="opacity-0 group-hover/turn:opacity-100 transition-opacity flex-shrink-0"
          />
        )}

        {isRunning && onCancel ? (
          <div
            data-testid="agent-turn-cancel-area"
            className="group/cancel relative flex items-center justify-center w-7 h-7 flex-shrink-0"
          >
            <Loader2
              data-testid="event-running-icon"
              className="w-3.5 h-3.5 animate-spin transition-opacity group-hover/cancel:opacity-0"
              style={{ color: COLORS.textMuted }}
            />
            <button
              data-testid="agent-turn-cancel"
              type="button"
              onClick={onCancel}
              className="absolute inset-0 flex items-center justify-center rounded hover:bg-black/[0.04] transition-colors opacity-0 pointer-events-none group-hover/cancel:opacity-100 group-hover/cancel:pointer-events-auto"
              title="Pause"
            >
              <Pause className="w-3.5 h-3.5" style={{ color: COLORS.textMuted }} />
            </button>
          </div>
        ) : isCancelled || isErrored ? (
          <div className="relative flex items-center justify-center w-3.5 h-3.5 flex-shrink-0">
            <div className="absolute inset-0 rounded-full" style={{ backgroundColor: `${COLORS.warning}33` }} />
            <X className="w-2.5 h-2.5" style={{ color: COLORS.warning }} />
          </div>
        ) : (
          <CheckCircle2 className="w-3.5 h-3.5 flex-shrink-0" style={{ color: COLORS.accent }} />
        )}
      </div>

      {hasCollapsedContent && (
        <div
          data-testid="activity-agent-message-content"
          className="luban-font-chat mt-2"
          style={{ fontSize: "15px", fontWeight: 400, lineHeight: "24px", color: COLORS.textPrimary }}
        >
          <Markdown content={collapsedMessageContent} enableMermaid />
        </div>
      )}

      {isExpanded && (
        <div className="mt-2 space-y-0.5">
          {activities.length === 0 ? (
            <div style={{ fontSize: "12px", color: COLORS.textMuted }}>No activity yet.</div>
          ) : (
            activities.map((event) => {
              const eventTestId = event.type === "assistant_message" ? "agent-turn-message-event" : "agent-turn-event"
              return (
                <div key={event.id} data-testid={eventTestId}>
                  <ActivityEventItem
                    event={event}
                    isExpanded={expandedEvents.has(event.id)}
                    onToggle={() => toggleEvent(event.id)}
                    duration={durationLabel(event)}
                  />
                </div>
              )
            })
          )}
        </div>
      )}
    </div>
  )
}

interface TaskHeaderSectionProps {
  title: string
  description?: string
  onTitleChange?: (title: string) => void
  onDescriptionChange?: (description: string) => void
}

function TaskHeaderSection({
  title,
  description,
  onTitleChange,
  onDescriptionChange,
}: TaskHeaderSectionProps) {
  return (
    <div style={{ padding: '32px 0px 16px 0px' }}>
      {/* Title - Linear style: 24px, 600, line-height 38.4px, letter-spacing -0.1px */}
      <div
        className="outline-none"
        style={{ 
          fontSize: '24px', 
          fontWeight: 600, 
          lineHeight: '38.4px',
          letterSpacing: '-0.1px',
          color: COLORS.textPrimary,
          marginBottom: '14px'
        }}
        contentEditable={!!onTitleChange}
        suppressContentEditableWarning
        onBlur={(e) => onTitleChange?.(e.currentTarget.textContent || "")}
      >
        {title || "Untitled Task"}
      </div>

      {/* Description - Linear style: 15px, 450, line-height 24px */}
      {(description || onDescriptionChange) && (
        <div
          className="outline-none min-h-[24px]"
          style={{ 
            fontSize: '15px', 
            fontWeight: 450,
            lineHeight: '24px',
            color: COLORS.textSecondary 
          }}
          contentEditable={!!onDescriptionChange}
          suppressContentEditableWarning
          onBlur={(e) => onDescriptionChange?.(e.currentTarget.textContent || "")}
          data-placeholder="Add a description..."
        >
          {description || ""}
        </div>
      )}
    </div>
  )
}

interface ActivityStreamSectionProps {
  listKey: string
  scrollElement: HTMLElement | null
  messages: Message[]
  isLoading?: boolean
}

type ActivityGroup = 
  | { type: "events"; messages: Message[]; startIndex: number }
  | { type: "message"; message: Message }

function groupMessages(messages: Message[]): ActivityGroup[] {
  const groups: ActivityGroup[] = []
  let currentEventGroup: Message[] = []
  let eventGroupStartIndex = 0

  for (let i = 0; i < messages.length; i++) {
    const message = messages[i]
    if (message.type === "event") {
      if (currentEventGroup.length === 0) {
        eventGroupStartIndex = i
      }
      currentEventGroup.push(message)
    } else {
      if (currentEventGroup.length > 0) {
        groups.push({ type: "events", messages: currentEventGroup, startIndex: eventGroupStartIndex })
        currentEventGroup = []
      }
      groups.push({ type: "message", message })
    }
  }
  
  if (currentEventGroup.length > 0) {
    groups.push({ type: "events", messages: currentEventGroup, startIndex: eventGroupStartIndex })
  }
  
  return groups
}

function ActivityStreamSection({
  listKey,
  scrollElement,
  messages,
  isLoading,
  onCancelAgentTurn,
  workspaceId,
  taskId,
}: ActivityStreamSectionProps & { onCancelAgentTurn?: () => void; workspaceId?: number; taskId?: number }) {
  const [expandedGroups, setExpandedGroups] = useState<Set<number>>(new Set())
  const [agentTurnUiStateById, setAgentTurnUiStateById] = useState<
    Map<string, { isExpanded: boolean; expandedEvents: Set<string> }>
  >(new Map())
  const groups = useMemo(() => groupMessages(messages), [messages])

  useEffect(() => {
    setExpandedGroups(new Set())
    setAgentTurnUiStateById(new Map())
  }, [listKey])

  const toggleGroup = useCallback((startIndex: number) => {
    setExpandedGroups((prev) => {
      const next = new Set(prev)
      if (next.has(startIndex)) {
        next.delete(startIndex)
      } else {
        next.add(startIndex)
      }
      return next
    })
  }, [])

  const items = useMemo<WindowedListItem[]>(() => {
    return groups.map((group) => {
      if (group.type === "message") {
        const msg = group.message
        if (msg.type === "terminal_command") {
          return {
            key: msg.id,
            node: (
              <div className="relative">
                <TerminalCommandCardEvent message={msg} workspaceId={workspaceId} taskId={taskId} />
              </div>
            ),
          }
        }
        if (msg.type === "task_status_suggestion") {
          return {
            key: msg.id,
            node: (
              <div className="relative">
                <TaskStatusSuggestionCardEvent message={msg} workspaceId={workspaceId} taskId={taskId} />
              </div>
            ),
          }
        }
        if (msg.type === "user") {
          return {
            key: msg.id,
            node: (
              <div className="relative">
                <UserActivityEvent message={msg} workspaceId={workspaceId} />
              </div>
            ),
          }
        }

        if (msg.type === "assistant") {
          return {
            key: msg.id,
            node: (
              <div className="relative">
                <AgentActivityEvent message={msg} />
              </div>
            ),
          }
        }

        if (msg.type === "agent_turn") {
          const state = agentTurnUiStateById.get(msg.id) ?? {
            isExpanded: false,
            expandedEvents: new Set<string>(),
          }
          return {
            key: msg.id,
            node: (
              <div className="relative">
                <AgentTurnCardEvent
                  message={msg}
                  onCancel={msg.turnStatus === "running" ? onCancelAgentTurn : undefined}
                  isExpanded={state.isExpanded}
                  expandedEvents={state.expandedEvents}
                  onExpandedChange={(nextExpanded) => {
                    setAgentTurnUiStateById((prev) => {
                      const next = new Map(prev)
                      const prevState = next.get(msg.id) ?? { isExpanded: false, expandedEvents: new Set<string>() }
                      next.set(msg.id, { ...prevState, isExpanded: nextExpanded })
                      return next
                    })
                  }}
                  onToggleEvent={(eventId) => {
                    setAgentTurnUiStateById((prev) => {
                      const next = new Map(prev)
                      const prevState = next.get(msg.id) ?? { isExpanded: false, expandedEvents: new Set<string>() }
                      const nextEvents = new Set(prevState.expandedEvents)
                      if (nextEvents.has(eventId)) nextEvents.delete(eventId)
                      else nextEvents.add(eventId)
                      next.set(msg.id, { ...prevState, expandedEvents: nextEvents })
                      return next
                    })
                  }}
                />
              </div>
            ),
          }
        }

        return { key: msg.id, node: null }
      }

      const isExpanded = expandedGroups.has(group.startIndex)
      const shouldCollapse = group.messages.length > 3 && !isExpanded

      if (shouldCollapse) {
        return {
          key: `event-group:${group.startIndex}`,
          node: (
            <div className="relative">
              <CollapsedEventsGroup events={group.messages} onExpand={() => toggleGroup(group.startIndex)} />
            </div>
          ),
        }
      }

      return {
        key: `event-group:${group.startIndex}:expanded:${isExpanded ? "1" : "0"}`,
        node: (
          <div className="flex flex-col gap-2">
            {group.messages.map((message, index) => {
              const hasNextEvent = index < group.messages.length - 1
              return (
                <div key={message.id} className="relative">
                  {hasNextEvent && (
                    <div
                      className="absolute"
                      style={{
                        left: `${ACTIVITY_TIMELINE_LEFT_PX}px`,
                        top: "20px",
                        bottom: "-8px",
                        width: "1px",
                        backgroundColor: COLORS.timeline,
                      }}
                    />
                  )}
                  <SystemEventItem message={message} />
                </div>
              )
            })}
          </div>
        ),
      }
    })
  }, [agentTurnUiStateById, expandedGroups, groups, onCancelAgentTurn, taskId, toggleGroup, workspaceId])

  const shouldWindow = items.length > 200

  return (
    <div>
      {/* Activity header - Linear style: 15px, 600 */}
      <div 
        className="flex items-center justify-between"
        style={{ 
          padding: '12px 0px',
          borderTop: `1px solid ${COLORS.border}`
        }}
      >
        <h3 
          style={{ 
            fontSize: '15px', 
            fontWeight: 600, 
            color: COLORS.textPrimary 
          }}
        >
          Activity
        </h3>
      </div>

      {/* Activity list (Linear style) */}
      <div className="py-4">
        {messages.length === 0 && !isLoading && (
          <div 
            className="py-8 text-center"
            style={{ fontSize: '13px', color: COLORS.textMuted }}
          >
            No activity yet. Start a conversation below.
          </div>
        )}

        {shouldWindow ? (
          <WindowedList
            items={items}
            listKey={listKey}
            scrollElement={scrollElement}
            itemClassName="pb-2"
            estimatedItemHeightPx={160}
            overscanPx={1000}
          />
        ) : (
          <div className="flex flex-col gap-2">
            {items.map((it) => (
              <div key={it.key}>{it.node}</div>
            ))}
          </div>
        )}


      </div>
    </div>
  )
}

export interface TaskActivityViewProps {
  listKey: string
  title: string
  description?: string
  workspaceId?: number
  taskId?: number
  messages: Message[]
  isLoading?: boolean
  onTitleChange?: (title: string) => void
  onDescriptionChange?: (description: string) => void
  inputComponent?: React.ReactNode
  onCancelAgentTurn?: () => void
  className?: string
}

export function TaskActivityView({
  listKey,
  title,
  description,
  workspaceId,
  taskId,
  messages,
  isLoading,
  onTitleChange,
  onDescriptionChange,
  inputComponent,
  onCancelAgentTurn,
  className,
}: TaskActivityViewProps) {
  const scrollContainerRef = useRef<HTMLDivElement | null>(null)
  const [scrollContainerEl, setScrollContainerEl] = useState<HTMLDivElement | null>(null)
  const setScrollContainer = useCallback((el: HTMLDivElement | null) => {
    scrollContainerRef.current = el
    setScrollContainerEl(el)
  }, [])
  const contentWrapperRef = useRef<HTMLDivElement | null>(null)
  const [contentWrapperEl, setContentWrapperEl] = useState<HTMLDivElement | null>(null)
  const setContentWrapper = useCallback((el: HTMLDivElement | null) => {
    contentWrapperRef.current = el
    setContentWrapperEl(el)
  }, [])
  const autoScrollRef = useRef<{ epoch: number; untilMs: number; maxUntilMs: number; raf: number | null } | null>(null)

  const stopAutoScrollToLatest = useCallback(() => {
    const pending = autoScrollRef.current
    if (pending?.raf != null) {
      window.cancelAnimationFrame(pending.raf)
    }
    autoScrollRef.current = null
  }, [])

  const startAutoScrollToLatest = useCallback(
    ({ durationMs, maxDurationMs }: { durationMs: number; maxDurationMs: number }) => {
      if (workspaceId == null || taskId == null) return
      const el = scrollContainerRef.current
      if (!el) return

      const prev = autoScrollRef.current
      const epoch = (prev?.epoch ?? 0) + 1
      const startedAt = Date.now()
      const maxUntilMs = startedAt + maxDurationMs
      const untilMs = Math.min(maxUntilMs, startedAt + durationMs)

      if (prev?.raf != null) window.cancelAnimationFrame(prev.raf)
      autoScrollRef.current = { epoch, untilMs, maxUntilMs, raf: null }

      const tick = () => {
        const pending = autoScrollRef.current
        if (!pending || pending.epoch !== epoch) return
        const root = scrollContainerRef.current
        if (root) {
          root.scrollTop = root.scrollHeight
        }
        if (Date.now() >= pending.untilMs) {
          stopAutoScrollToLatest()
          return
        }
        pending.raf = window.requestAnimationFrame(tick)
      }

      autoScrollRef.current.raf = window.requestAnimationFrame(tick)
    },
    [stopAutoScrollToLatest, taskId, workspaceId],
  )

  useEffect(() => {
    if (!scrollContainerEl) return
    if (workspaceId == null || taskId == null) return
    startAutoScrollToLatest({ durationMs: 400, maxDurationMs: 2000 })
    return () => stopAutoScrollToLatest()
  }, [listKey, scrollContainerEl, startAutoScrollToLatest, stopAutoScrollToLatest, taskId, workspaceId])

  useEffect(() => {
    const el = scrollContainerEl
    if (!el) return
    const onUserScrollIntent = () => stopAutoScrollToLatest()
    el.addEventListener("wheel", onUserScrollIntent, { passive: true })
    el.addEventListener("touchstart", onUserScrollIntent, { passive: true })
    el.addEventListener("pointerdown", onUserScrollIntent)
    return () => {
      el.removeEventListener("wheel", onUserScrollIntent)
      el.removeEventListener("touchstart", onUserScrollIntent)
      el.removeEventListener("pointerdown", onUserScrollIntent)
    }
  }, [scrollContainerEl, stopAutoScrollToLatest])

  useEffect(() => {
    const wrapper = contentWrapperEl
    if (!wrapper) return
    const observer = new ResizeObserver(() => {
      const pending = autoScrollRef.current
      if (!pending) return
      const now = Date.now()
      if (now >= pending.maxUntilMs) return
      pending.untilMs = Math.min(pending.maxUntilMs, now + 250)
    })
    observer.observe(wrapper)
    return () => observer.disconnect()
  }, [contentWrapperEl, listKey])

  return (
    <div 
      className={cn("flex flex-col h-full", className)} 
      style={{ backgroundColor: COLORS.background }}
    >
      {/* Scrollable content */}
      <div
        ref={setScrollContainer}
        className="flex-1 overflow-y-auto"
        data-testid="chat-scroll-container"
        style={{ padding: "0 60px" }}
      >
        <div
          ref={setContentWrapper}
          data-testid="chat-content-wrapper"
          style={{ maxWidth: "686px", margin: "0 auto" }}
        >
          {/* Task header with title and description */}
          <TaskHeaderSection
            title={title}
            description={description}
            onTitleChange={onTitleChange}
            onDescriptionChange={onDescriptionChange}
          />

          {/* Activity stream */}
          <ActivityStreamSection
            messages={messages}
            isLoading={isLoading}
            onCancelAgentTurn={onCancelAgentTurn}
            workspaceId={workspaceId}
            taskId={taskId}
            listKey={listKey}
            scrollElement={scrollContainerEl}
          />
          
          {/* Input at bottom of activity - aligned with cards */}
          {inputComponent && (
            <div 
              style={{ 
                marginLeft: '-6px',
                marginRight: '-6px',
                marginTop: '18px'
              }}
            >
              {inputComponent}
            </div>
          )}
          
          {/* Bottom padding for scroll */}
          <div style={{ height: '60px' }} />
        </div>
      </div>
    </div>
  )
}
