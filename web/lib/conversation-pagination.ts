"use client"

import type { ConversationEntry, ConversationSnapshot } from "./luban-api"

type Slice = {
  start: number
  end: number
  entries: ConversationEntry[]
}

function entriesTotal(snapshot: ConversationSnapshot): number {
  return snapshot.entries_total ?? snapshot.entries.length
}

function entriesStart(snapshot: ConversationSnapshot): number {
  return snapshot.entries_start ?? 0
}

function sliceFromSnapshot(snapshot: ConversationSnapshot): Slice {
  const start = entriesStart(snapshot)
  const end = start + snapshot.entries.length
  return { start, end, entries: snapshot.entries }
}

function isSameThread(a: ConversationSnapshot, b: ConversationSnapshot): boolean {
  return a.workdir_id === b.workdir_id && a.task_id === b.task_id
}

function computeTruncation(start: number, entries: ConversationEntry[], total: number): boolean {
  return start > 0 || start + entries.length < total
}

export function prependConversationSnapshot(
  current: ConversationSnapshot,
  olderPage: ConversationSnapshot,
): ConversationSnapshot {
  if (!isSameThread(current, olderPage)) return current

  const currentSlice = sliceFromSnapshot(current)
  const olderSlice = sliceFromSnapshot(olderPage)

  if (olderSlice.start >= currentSlice.start) return current

  const overlap = Math.max(0, olderSlice.end - currentSlice.start)
  const olderPrefix = overlap > 0 ? olderSlice.entries.slice(0, olderSlice.entries.length - overlap) : olderSlice.entries

  const mergedStart = olderSlice.start
  const mergedEntries = olderPrefix.concat(currentSlice.entries)
  const total = Math.max(entriesTotal(current), entriesTotal(olderPage))

  return {
    ...current,
    entries_total: total,
    entries_start: mergedStart,
    entries: mergedEntries,
    entries_truncated: computeTruncation(mergedStart, mergedEntries, total),
  }
}

export function compactConversationToTail(
  snapshot: ConversationSnapshot,
  limit: number,
): ConversationSnapshot {
  const clamped = Math.max(1, Math.min(5000, Math.floor(limit)))
  const total = entriesTotal(snapshot)
  const start = entriesStart(snapshot)
  if (snapshot.entries.length <= clamped && start + snapshot.entries.length >= total) {
    return snapshot
  }

  const desiredEnd = Math.min(total, start + snapshot.entries.length)
  const desiredStart = Math.max(0, desiredEnd - clamped)
  const offset = Math.max(0, desiredStart - start)
  const tailEntries = snapshot.entries.slice(offset)

  return {
    ...snapshot,
    entries_start: desiredStart,
    entries: tailEntries,
    entries_truncated: computeTruncation(desiredStart, tailEntries, total),
  }
}
