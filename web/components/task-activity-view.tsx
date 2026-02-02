"use client"

import type React from "react"
import { useState, useCallback, useRef } from "react"
import {
  Brain,
  Check,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  ChevronsUpDown,
  Clock,
  Copy,
  FileCode,
  Loader2,
  Pencil,
  Play,
  Plus,
  Terminal,
  Wrench,
  Eye,
  X,
} from "lucide-react"

import { cn } from "@/lib/utils"
import type { Message, ActivityEvent } from "@/lib/conversation-ui"
import { Markdown } from "@/components/markdown"
import { AnsiOutput } from "@/components/shared/ansi-output"
import { useActivityTiming } from "@/lib/activity-timing"

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
  const hasExpandableDetail = event.type === "bash" || detail.trim().length > 0

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
      default:
        return <Wrench className="w-3.5 h-3.5" />
    }
  })()

  return (
    <div className="group/event">
      <button
        onClick={() => hasExpandableDetail && onToggle()}
        className={cn(
          "w-full flex items-center gap-2 py-1 px-2 -mx-2 rounded transition-colors",
          hasExpandableDetail ? "hover:bg-black/[0.03] cursor-pointer" : "cursor-default"
        )}
        style={{ 
          fontSize: '12px',
          fontWeight: 450,
          color: event.status === "running" ? COLORS.accent : COLORS.textMuted 
        }}
      >
        {event.status === "running" ? (
          <Loader2 className="w-3.5 h-3.5 animate-spin flex-shrink-0" />
        ) : (
          <span className="flex-shrink-0">{icon}</span>
        )}
        <span className="flex-1 text-left truncate">{event.title}</span>
        <span 
          className="font-mono tabular-nums text-right min-w-[52px]"
          style={{ fontSize: '10px', color: COLORS.textMuted }}
        >
          {duration ?? ""}
        </span>
        {hasExpandableDetail && (
          <ChevronRight
            className={cn(
              "w-3 h-3 flex-shrink-0 transition-transform",
              isExpanded && "rotate-90"
            )}
            style={{ color: COLORS.textMuted }}
          />
        )}
      </button>
      {isExpanded && hasExpandableDetail && (
        <div
          className="ml-5 pl-3 py-1.5 mb-1 font-mono"
          style={{ 
            borderLeft: `1px solid ${COLORS.timeline}`, 
            fontSize: '11px',
            color: COLORS.textMuted 
          }}
        >
          <AnsiOutput text={detail} />
        </div>
      )}
    </div>
  )
}

interface CollapsibleActivitiesProps {
  activities: ActivityEvent[]
  isStreaming?: boolean
  isCancelled?: boolean
}

function CollapsibleActivities({ activities, isStreaming, isCancelled }: CollapsibleActivitiesProps) {
  const [isExpanded, setIsExpanded] = useState(false)
  const [expandedEvents, setExpandedEvents] = useState<Set<string>>(new Set())
  const { durationLabel } = useActivityTiming(activities)

  const latestActivity = activities[activities.length - 1]
  const completedCount = activities.filter((a) => a.status === "done" && a.title !== "Turn canceled").length

  const toggleEvent = (eventId: string) => {
    setExpandedEvents((prev) => {
      const next = new Set(prev)
      if (next.has(eventId)) {
        next.delete(eventId)
      } else {
        next.add(eventId)
      }
      return next
    })
  }

  if (!activities.length) return null

  return (
    <div className="mb-1.5">
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="flex items-center gap-2 py-1 px-2 -mx-2 rounded transition-colors w-full hover:bg-black/[0.03]"
        style={{ 
          fontSize: '12px',
          fontWeight: 450,
          color: isStreaming ? COLORS.accent : COLORS.textMuted 
        }}
      >
        {isStreaming && latestActivity?.status === "running" ? (
          <Loader2 className="w-3.5 h-3.5 animate-spin flex-shrink-0" />
        ) : isCancelled ? (
          <div className="relative flex items-center justify-center w-3.5 h-3.5 flex-shrink-0">
            <div 
              className="absolute inset-0 rounded-full" 
              style={{ backgroundColor: `${COLORS.warning}33` }} 
            />
            <X className="w-2.5 h-2.5" style={{ color: COLORS.warning }} />
          </div>
        ) : (
          <CheckCircle2 className="w-3.5 h-3.5 flex-shrink-0" style={{ color: COLORS.accent }} />
        )}
        <span className="flex-1 text-left truncate">
          {isStreaming
            ? latestActivity?.title
            : isCancelled
              ? `Cancelled after ${completedCount} steps`
              : `Completed ${completedCount} steps`}
        </span>
        <ChevronDown
          className={cn("w-3.5 h-3.5 transition-transform", isExpanded && "rotate-180")}
          style={{ color: COLORS.textMuted }}
        />
      </button>

      {isExpanded && (
        <div 
          className="mt-1 ml-1 pl-3 space-y-0.5" 
          style={{ borderLeft: `1px solid ${COLORS.timeline}` }}
        >
          {activities.map((event) => (
            <ActivityEventItem
              key={event.id}
              event={event}
              isExpanded={expandedEvents.has(event.id)}
              onToggle={() => toggleEvent(event.id)}
              duration={durationLabel(event)}
            />
          ))}
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
      return { name: "Agent", initial: "A", color: COLORS.textPrimary }
    }
    if (message.eventSource === "system") {
      return { name: "System", initial: "S", color: COLORS.textMuted }
    }
    return { name: "You", initial: "U", color: COLORS.accent }
  })()
  const eventActor = actor || defaultActor

  return (
    <div 
      className="flex items-start"
      style={{ padding: '1px 0' }}
      data-testid="activity-event"
    >
      {/* Icon column - 14x14 circular icon, centered with card avatars */}
      {/* Card avatar center: -6px margin + 16px padding + 10px radius = 20px from container edge */}
      {/* Event icon center should be at 20px: marginLeft + 7px = 20px, so marginLeft = 13px */}
      <div
        className="flex items-center justify-center flex-shrink-0 relative z-10"
        style={{ 
          width: '14px', 
          height: '16.8px',
          marginLeft: '13px',  /* Align icon center (13+7=20px) with card avatar center */
          marginRight: '4px',
          backgroundColor: COLORS.background
        }}
      >
        <div
          data-testid="event-avatar"
          className="flex items-center justify-center text-white"
          style={{ 
            width: '14px', 
            height: '14px', 
            borderRadius: '50%',
            backgroundColor: eventActor.color,
            fontSize: '7px',
            fontWeight: 500
          }}
        >
          {eventActor.initial}
        </div>
      </div>
      
      {/* Event text - Linear style: 12px, muted colors, inline */}
      <span 
        data-testid="event-text"
        className="flex items-center flex-wrap"
        style={{ fontSize: '12px', lineHeight: '16.8px', color: COLORS.textMuted }}
      >
        <b style={{ fontWeight: 500 }}>{eventActor.name}</b>
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
            width: '14px', 
            marginLeft: '13px',
            marginRight: '4px',
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
                    left: '19.5px',
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
}

function UserActivityEvent({ message }: UserActivityEventProps) {
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
      <div className="flex items-center gap-2.5 mb-2">
        {/* Avatar - 20x20 circular (Linear style) */}
        <div
          className="flex items-center justify-center text-white flex-shrink-0"
          style={{ 
            width: '20px', 
            height: '20px', 
            borderRadius: '50%',
            backgroundColor: COLORS.accent,
            fontSize: '9px',
            fontWeight: 500
          }}
        >
          U
        </div>
        <span style={{ fontSize: '14px', fontWeight: 500, color: COLORS.textPrimary }}>
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

interface AgentActivityEventProps {
  message: Message
}

function AgentActivityEvent({ message }: AgentActivityEventProps) {
  const activities = message.activities ?? []
  const hasContent = message.content.length > 0

  return (
    <div className="flex flex-col gap-2">
      {activities.length > 0 && (
        <CollapsibleActivities
          activities={activities}
          isStreaming={message.isStreaming}
          isCancelled={message.isCancelled}
        />
      )}

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
          <div className="flex items-center gap-2.5 mb-2">
            <div
              className="flex items-center justify-center text-white flex-shrink-0"
              style={{ 
                width: '20px', 
                height: '20px', 
                borderRadius: '50%',
                backgroundColor: COLORS.textPrimary,
                fontSize: '9px',
                fontWeight: 500
              }}
            >
              A
            </div>
            <span style={{ fontSize: '14px', fontWeight: 500, color: COLORS.textPrimary }}>
              Agent
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

function ActivityStreamSection({ messages, isLoading }: ActivityStreamSectionProps) {
  const [expandedGroups, setExpandedGroups] = useState<Set<number>>(new Set())
  const groups = groupMessages(messages)

  const toggleGroup = (startIndex: number) => {
    setExpandedGroups(prev => {
      const next = new Set(prev)
      if (next.has(startIndex)) {
        next.delete(startIndex)
      } else {
        next.add(startIndex)
      }
      return next
    })
  }

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

        {/* Activity items - 8px gap between simple events */}
        <div className="flex flex-col gap-2">
          {groups.map((group, groupIndex) => {
            if (group.type === "message") {
              return (
                <div key={group.message.id} className="relative">
                  {group.message.type === "user" ? (
                    <UserActivityEvent message={group.message} />
                  ) : (
                    <AgentActivityEvent message={group.message} />
                  )}
                </div>
              )
            }

            // Event group
            const isExpanded = expandedGroups.has(group.startIndex)
            const shouldCollapse = group.messages.length > 3 && !isExpanded

            if (shouldCollapse) {
              return (
                <div key={`event-group-${group.startIndex}`} className="relative">
                  <CollapsedEventsGroup 
                    events={group.messages} 
                    onExpand={() => toggleGroup(group.startIndex)} 
                  />
                </div>
              )
            }

            return (
              <div key={`event-group-${group.startIndex}`} className="flex flex-col gap-2">
                {group.messages.map((message, index) => {
                  const hasNextEvent = index < group.messages.length - 1
                  return (
                    <div key={message.id} className="relative">
                      {/* Timeline connector - only between adjacent events */}
                      {hasNextEvent && (
                        <div 
                          className="absolute"
                          style={{
                            left: '19.5px',
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
            )
          })}
        </div>


      </div>
    </div>
  )
}

export interface TaskActivityViewProps {
  title: string
  description?: string
  messages: Message[]
  isLoading?: boolean
  onTitleChange?: (title: string) => void
  onDescriptionChange?: (description: string) => void
  inputComponent?: React.ReactNode
  className?: string
}

export function TaskActivityView({
  title,
  description,
  messages,
  isLoading,
  onTitleChange,
  onDescriptionChange,
  inputComponent,
  className,
}: TaskActivityViewProps) {
  const scrollContainerRef = useRef<HTMLDivElement>(null)

  return (
    <div 
      className={cn("flex flex-col h-full", className)} 
      style={{ backgroundColor: COLORS.background }}
    >
      {/* Scrollable content */}
      <div ref={scrollContainerRef} className="flex-1 overflow-y-auto" data-testid="chat-scroll-container">
        <div style={{ maxWidth: '686px', margin: '0 60px' }}>
          {/* Task header with title and description */}
          <TaskHeaderSection
            title={title}
            description={description}
            onTitleChange={onTitleChange}
            onDescriptionChange={onDescriptionChange}
          />

          {/* Activity stream */}
          <ActivityStreamSection messages={messages} isLoading={isLoading} />
          
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
