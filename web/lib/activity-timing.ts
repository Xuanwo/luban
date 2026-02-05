"use client"

import { useEffect, useState } from "react"

import type { ActivityEvent } from "@/lib/conversation-ui"
import { formatDurationMs } from "@/lib/duration-format"

const TURN_DURATION_TITLE_PREFIX = "Turn duration:"

function formatStepDuration(ms: number): string {
  return formatDurationMs(ms)
}

export function extractTurnDurationLabel(activities: ActivityEvent[]): string | null {
  for (let idx = activities.length - 1; idx >= 0; idx -= 1) {
    const event = activities[idx]
    if (!event) continue
    const title = event.title ?? ""
    if (!title.startsWith(TURN_DURATION_TITLE_PREFIX)) continue
    const label = title.slice(TURN_DURATION_TITLE_PREFIX.length).trim()
    return label.length > 0 ? label : null
  }
  return null
}

export function useActivityTiming(
  activities: ActivityEvent[],
): {
  durationLabel: (event: ActivityEvent) => string | null
} {
  const [tick, setTick] = useState(0)

  const hasRunningSteps = activities.some((e) => e.status === "running" && e.timing?.startedAtUnixMs != null)
  useEffect(() => {
    if (!hasRunningSteps) return
    const timer = window.setInterval(() => setTick((n) => (n + 1) % 1_000_000), 250)
    return () => window.clearInterval(timer)
  }, [hasRunningSteps])

  const durationLabel = (event: ActivityEvent): string | null => {
    void tick
    const startedAtUnixMs = event.timing?.startedAtUnixMs ?? null
    const doneAtUnixMs = event.timing?.doneAtUnixMs ?? null
    if (typeof startedAtUnixMs !== "number" || !Number.isFinite(startedAtUnixMs) || startedAtUnixMs <= 0) return null

    const end =
      typeof doneAtUnixMs === "number" && Number.isFinite(doneAtUnixMs) && doneAtUnixMs > 0
        ? doneAtUnixMs
        : event.status === "running"
          ? Date.now()
          : null
    if (end == null) return null

    return formatStepDuration(Math.max(0, end - startedAtUnixMs))
  }

  return { durationLabel }
}
