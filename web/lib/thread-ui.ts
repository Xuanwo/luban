"use client"

import type { ThreadMeta } from "./luban-api"

export function pickThreadId(args: { threads: ThreadMeta[]; preferredThreadId: number | null }): number | null {
  const threads = args.threads
  if (threads.length === 0) return null

  if (args.preferredThreadId != null && threads.some((t) => t.task_id === args.preferredThreadId)) {
    return args.preferredThreadId
  }

  const mostRecent = threads
    .slice()
    .sort((a, b) => (b.updated_at_unix_seconds ?? 0) - (a.updated_at_unix_seconds ?? 0))[0]
  return mostRecent?.task_id ?? threads[0]?.task_id ?? null
}
