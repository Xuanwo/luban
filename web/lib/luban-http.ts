import type {
  AppSnapshot,
  AttachmentKind,
  AttachmentRef,
  CodexCustomPromptSnapshot,
  ContextSnapshot,
  ConversationSnapshot,
  MentionItemSnapshot,
  ThreadsSnapshot,
  WorkspaceChangesSnapshot,
  WorkspaceDiffSnapshot,
} from "./luban-api"
import { isMockMode } from "./luban-mode"
import {
  mockDeleteContextItem,
  mockFetchApp,
  mockFetchCodexCustomPrompts,
  mockFetchContext,
  mockFetchConversation,
  mockFetchMentionItems,
  mockFetchThreads,
  mockFetchWorkspaceChanges,
  mockFetchWorkspaceDiff,
  mockUploadAttachment,
} from "./mock/mock-runtime"

export async function fetchApp(): Promise<AppSnapshot> {
  if (isMockMode()) return await mockFetchApp()
  const res = await fetch("/api/app")
  if (!res.ok) throw new Error(`GET /api/app failed: ${res.status}`)
  return (await res.json()) as AppSnapshot
}

export async function fetchThreads(workspaceId: number): Promise<ThreadsSnapshot> {
  if (isMockMode()) return await mockFetchThreads(workspaceId)
  const res = await fetch(`/api/workspaces/${workspaceId}/threads`)
  if (!res.ok) throw new Error(`GET /api/workspaces/${workspaceId}/threads failed: ${res.status}`)
  return (await res.json()) as ThreadsSnapshot
}

export async function fetchConversation(
  workspaceId: number,
  threadId: number,
  args: { before?: number; limit?: number } = {},
): Promise<ConversationSnapshot> {
  if (isMockMode()) return await mockFetchConversation(workspaceId, threadId, args)
  const limit = args.limit ?? 2000
  const params = new URLSearchParams({ limit: String(limit) })
  if (args.before != null) params.set("before", String(args.before))
  const res = await fetch(`/api/workspaces/${workspaceId}/conversations/${threadId}?${params.toString()}`)
  if (!res.ok)
    throw new Error(
      `GET /api/workspaces/${workspaceId}/conversations/${threadId} failed: ${res.status}`,
    )
  return (await res.json()) as ConversationSnapshot
}

export async function uploadAttachment(args: {
  workspaceId: number
  file: File
  kind: AttachmentKind
  idempotencyKey?: string
}): Promise<AttachmentRef> {
  if (isMockMode()) return await mockUploadAttachment(args)
  const form = new FormData()
  form.append("kind", args.kind)
  form.append("file", args.file, args.file.name)

  const resolvedKey =
    args.idempotencyKey ??
    (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function"
      ? crypto.randomUUID()
      : `att_${Math.random().toString(16).slice(2)}_${Date.now().toString(16)}`)

  let res: Response
  try {
    res = await fetch(`/api/workspaces/${args.workspaceId}/attachments`, {
      method: "POST",
      headers: { "Idempotency-Key": resolvedKey },
      body: form,
    })
  } catch {
    await new Promise((r) => window.setTimeout(r, 200))
    res = await fetch(`/api/workspaces/${args.workspaceId}/attachments`, {
      method: "POST",
      headers: { "Idempotency-Key": resolvedKey },
      body: form,
    })
  }
  if (!res.ok) {
    const text = await res.text().catch(() => "")
    throw new Error(
      `POST /api/workspaces/${args.workspaceId}/attachments failed: ${res.status}${text ? `: ${text}` : ""}`,
    )
  }

  return (await res.json()) as AttachmentRef
}

export async function fetchContext(workspaceId: number): Promise<ContextSnapshot> {
  if (isMockMode()) return await mockFetchContext(workspaceId)
  const res = await fetch(`/api/workspaces/${workspaceId}/context`)
  if (!res.ok) throw new Error(`GET /api/workspaces/${workspaceId}/context failed: ${res.status}`)
  return (await res.json()) as ContextSnapshot
}

export async function deleteContextItem(workspaceId: number, contextId: number): Promise<void> {
  if (isMockMode()) return await mockDeleteContextItem(workspaceId, contextId)
  const res = await fetch(`/api/workspaces/${workspaceId}/context/${contextId}`, { method: "DELETE" })
  if (res.status === 204) return
  if (!res.ok) {
    const text = await res.text().catch(() => "")
    throw new Error(
      `DELETE /api/workspaces/${workspaceId}/context/${contextId} failed: ${res.status}${text ? `: ${text}` : ""}`,
    )
  }
}

export async function fetchWorkspaceChanges(workspaceId: number): Promise<WorkspaceChangesSnapshot> {
  if (isMockMode()) return await mockFetchWorkspaceChanges(workspaceId)
  const res = await fetch(`/api/workspaces/${workspaceId}/changes`)
  if (!res.ok) throw new Error(`GET /api/workspaces/${workspaceId}/changes failed: ${res.status}`)
  return (await res.json()) as WorkspaceChangesSnapshot
}

export async function fetchWorkspaceDiff(workspaceId: number): Promise<WorkspaceDiffSnapshot> {
  if (isMockMode()) return await mockFetchWorkspaceDiff(workspaceId)
  const res = await fetch(`/api/workspaces/${workspaceId}/diff`)
  if (!res.ok) throw new Error(`GET /api/workspaces/${workspaceId}/diff failed: ${res.status}`)
  return (await res.json()) as WorkspaceDiffSnapshot
}

export async function fetchCodexCustomPrompts(): Promise<CodexCustomPromptSnapshot[]> {
  if (isMockMode()) return await mockFetchCodexCustomPrompts()
  const res = await fetch("/api/codex/prompts")
  if (!res.ok) throw new Error(`GET /api/codex/prompts failed: ${res.status}`)
  return (await res.json()) as CodexCustomPromptSnapshot[]
}

export async function fetchMentionItems(args: {
  workspaceId: number
  query: string
}): Promise<MentionItemSnapshot[]> {
  if (isMockMode()) return await mockFetchMentionItems(args)
  const q = args.query.trim()
  if (!q) return []
  const res = await fetch(
    `/api/workspaces/${args.workspaceId}/mentions?q=${encodeURIComponent(q)}`,
  )
  if (!res.ok) {
    const text = await res.text().catch(() => "")
    throw new Error(
      `GET /api/workspaces/${args.workspaceId}/mentions failed: ${res.status}${text ? `: ${text}` : ""}`,
    )
  }
  return (await res.json()) as MentionItemSnapshot[]
}
