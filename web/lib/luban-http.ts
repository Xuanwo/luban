import type { AppSnapshot, ConversationSnapshot, ThreadsSnapshot } from "./luban-api"

export async function fetchApp(): Promise<AppSnapshot> {
  const res = await fetch("/api/app")
  if (!res.ok) throw new Error(`GET /api/app failed: ${res.status}`)
  return (await res.json()) as AppSnapshot
}

export async function fetchThreads(workspaceId: number): Promise<ThreadsSnapshot> {
  const res = await fetch(`/api/workspaces/${workspaceId}/threads`)
  if (!res.ok) throw new Error(`GET /api/workspaces/${workspaceId}/threads failed: ${res.status}`)
  return (await res.json()) as ThreadsSnapshot
}

export async function fetchConversation(
  workspaceId: number,
  threadId: number,
): Promise<ConversationSnapshot> {
  const res = await fetch(`/api/workspaces/${workspaceId}/conversations/${threadId}`)
  if (!res.ok)
    throw new Error(
      `GET /api/workspaces/${workspaceId}/conversations/${threadId} failed: ${res.status}`,
    )
  return (await res.json()) as ConversationSnapshot
}
