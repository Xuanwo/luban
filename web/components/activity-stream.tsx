"use client"

import { useState } from "react"
import {
  Brain,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Eye,
  Loader2,
  Pencil,
  Terminal,
  Wrench,
  X,
} from "lucide-react"

import { cn } from "@/lib/utils"
import type { ActivityEvent } from "@/lib/conversation-ui"
import { useActivityTiming } from "@/lib/activity-timing"

function ActivityEventItem({
  event,
  isExpanded,
  onToggle,
  duration,
}: {
  event: ActivityEvent
  isExpanded: boolean
  onToggle: () => void
  duration: string | null
}) {
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
    <div className="group">
      <button
        onClick={() => {
          if (!hasExpandableDetail) return
          onToggle()
        }}
        className={cn(
          "relative w-full flex items-center gap-2 py-1 px-2 pr-[84px] -mx-2 rounded text-xs transition-colors overflow-hidden",
          hasExpandableDetail ? "hover:bg-muted/50" : "cursor-default",
          event.status === "running" ? "text-status-running" : "text-muted-foreground",
        )}
      >
        {event.status === "running" ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : icon}
        <span className="flex-1 text-left truncate">{event.title}</span>
        <div className="absolute right-1 top-1/2 -translate-y-1/2 flex items-center gap-1 pl-3">
          <div
            className={cn(
              "pointer-events-none absolute inset-y-0 -left-10 w-10",
              "bg-gradient-to-l from-background/95 via-background/95 to-transparent opacity-100 transition-opacity duration-200",
            )}
          />
          <div
            className={cn(
              "pointer-events-none absolute inset-y-0 -left-10 w-10",
              "bg-gradient-to-l from-muted/60 via-muted/60 to-transparent opacity-0 transition-opacity duration-200",
              "group-hover:opacity-100",
            )}
          />
          <span className="relative z-10 text-[10px] text-muted-foreground/70 font-mono tabular-nums text-right min-w-[52px]">
            {duration ?? ""}
          </span>
          {hasExpandableDetail && (
            <ChevronRight
              className={cn(
                "relative z-10 w-3 h-3 text-muted-foreground/50 transition-transform flex-shrink-0",
                isExpanded && "rotate-90",
              )}
            />
          )}
        </div>
      </button>
      {isExpanded && hasExpandableDetail && (
        <div className="ml-5 pl-2 border-l border-border text-[11px] text-muted-foreground py-1 mb-1">
          <pre className="whitespace-pre-wrap break-words font-mono">
            {detail.trim().length > 0 ? detail : "No output."}
          </pre>
        </div>
      )}
    </div>
  )
}

export function ActivityStream({
  activities,
  isStreaming,
  isCancelled = false,
}: { activities: ActivityEvent[]; isStreaming?: boolean; isCancelled?: boolean }) {
  const [isExpanded, setIsExpanded] = useState(false)
  const [expandedEvents, setExpandedEvents] = useState<Set<string>>(new Set())
  const { durationLabel } = useActivityTiming(activities)

  const latestActivity = activities[activities.length - 1]
  const completedCount = activities.filter((a) => a.status === "done" && a.title !== "Turn canceled").length

  const toggleEvent = (eventId: string) => {
    const nextExpanded = new Set(expandedEvents)
    if (nextExpanded.has(eventId)) {
      nextExpanded.delete(eventId)
    } else {
      nextExpanded.add(eventId)
    }
    setExpandedEvents(nextExpanded)
  }

  if (!activities.length) return null

  return (
    <div className="my-3">
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className={cn(
          "flex items-center gap-2 py-1.5 px-2 -mx-2 rounded text-xs transition-colors w-full",
          "hover:bg-muted/50",
          isStreaming ? "text-status-running" : "text-muted-foreground",
        )}
      >
        {isStreaming && latestActivity?.status === "running" ? (
          <Loader2 className="w-3.5 h-3.5 animate-spin flex-shrink-0" />
        ) : isCancelled ? (
          <div className="relative flex items-center justify-center w-3.5 h-3.5 flex-shrink-0">
            <div className="absolute inset-0 rounded-full bg-status-warning/20" />
            <X className="w-2.5 h-2.5 text-status-warning" />
          </div>
        ) : (
          <CheckCircle2 className="w-3.5 h-3.5 flex-shrink-0 text-status-success" />
        )}
        <span className="flex-1 text-left truncate">
          {isStreaming
            ? latestActivity?.title
            : isCancelled
              ? `Cancelled after ${completedCount} steps`
              : `Completed ${completedCount} steps`}
        </span>
        <ChevronDown
          className={cn("w-3.5 h-3.5 text-muted-foreground/50 transition-transform", isExpanded && "rotate-180")}
        />
      </button>

      {isExpanded && (
        <div className="mt-1 ml-1 pl-3 border-l-2 border-border/50 space-y-0.5">
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
