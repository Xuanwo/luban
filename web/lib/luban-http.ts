import type {
  AppSnapshot,
  AttachmentKind,
  AttachmentRef,
  CodexCustomPromptSnapshot,
  ConversationSnapshot,
  MentionItemSnapshot,
  NewTaskDraftSnapshot,
  NewTaskDraftsSnapshot,
  NewTaskStashResponse,
  TaskDocumentKind,
  TaskDocumentSnapshot,
  TaskDocumentsSnapshot,
  TaskStatus,
  TasksSnapshot,
  ThreadsSnapshot,
  WorkspaceChangesSnapshot,
  WorkspaceDiffSnapshot,
} from "./luban-api"
import { isMockMode } from "./luban-mode"
import {
  mockFetchApp,
  mockFetchCodexCustomPrompts,
  mockFetchConversation,
  mockFetchMentionItems,
  mockFetchTasks,
  mockFetchThreads,
  mockFetchWorkspaceDiff,
  mockCreateNewTaskDraft,
  mockDeleteNewTaskDraft,
  mockFetchNewTaskDrafts,
  mockFetchNewTaskStash,
  mockFetchTaskDocuments,
  mockFetchWorkspaceChanges,
  mockSaveNewTaskStash,
  mockUpdateTaskDocument,
  mockUpdateNewTaskDraft,
  mockClearNewTaskStash,
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
  const res = await fetch(`/api/workdirs/${workspaceId}/tasks`)
  if (!res.ok) throw new Error(`GET /api/workdirs/${workspaceId}/tasks failed: ${res.status}`)
  return (await res.json()) as ThreadsSnapshot
}

export async function fetchConversation(
  workspaceId: number,
  threadId: number,
  args: { before?: number; limit?: number } = {},
): Promise<ConversationSnapshot> {
  if (isMockMode()) return await mockFetchConversation(workspaceId, threadId, args)
  const limit = args.limit ?? 200
  const params = new URLSearchParams({ limit: String(limit) })
  if (args.before != null) params.set("before", String(args.before))
  const res = await fetch(`/api/workdirs/${workspaceId}/conversations/${threadId}?${params.toString()}`)
  if (!res.ok)
    throw new Error(
      `GET /api/workdirs/${workspaceId}/conversations/${threadId} failed: ${res.status}`,
    )
  return (await res.json()) as ConversationSnapshot
}

export async function fetchTaskDocuments(
  workspaceId: number,
  taskId: number,
): Promise<TaskDocumentsSnapshot> {
  if (isMockMode()) return await mockFetchTaskDocuments(workspaceId, taskId)
  const res = await fetch(`/api/workdirs/${workspaceId}/tasks/${taskId}/documents`)
  if (!res.ok) {
    const text = await res.text().catch(() => "")
    throw new Error(
      `GET /api/workdirs/${workspaceId}/tasks/${taskId}/documents failed: ${res.status}${text ? `: ${text}` : ""}`,
    )
  }
  return (await res.json()) as TaskDocumentsSnapshot
}

export async function fetchTaskDocument(args: {
  workspaceId: number
  taskId: number
  kind: TaskDocumentKind
}): Promise<TaskDocumentSnapshot> {
  if (isMockMode()) {
    const snapshot = await mockFetchTaskDocuments(args.workspaceId, args.taskId)
    const found = snapshot.documents.find((doc) => doc.kind === args.kind)
    if (found) return found
    return {
      kind: args.kind,
      rel_path: `tasks/v1/tasks/task-${args.workspaceId}-${args.taskId}/${args.kind.toUpperCase()}.md`,
      content: "",
      content_hash: "",
      byte_len: 0,
      updated_at_unix_ms: Date.now(),
    }
  }
  const res = await fetch(
    `/api/workdirs/${args.workspaceId}/tasks/${args.taskId}/documents/${args.kind}`,
  )
  if (!res.ok) {
    const text = await res.text().catch(() => "")
    throw new Error(
      `GET /api/workdirs/${args.workspaceId}/tasks/${args.taskId}/documents/${args.kind} failed: ${res.status}${text ? `: ${text}` : ""}`,
    )
  }
  return (await res.json()) as TaskDocumentSnapshot
}

export async function updateTaskDocument(args: {
  workspaceId: number
  taskId: number
  kind: TaskDocumentKind
  content: string
}): Promise<TaskDocumentSnapshot> {
  if (isMockMode()) {
    return await mockUpdateTaskDocument(args.workspaceId, args.taskId, args.kind, args.content)
  }
  const res = await fetch(
    `/api/workdirs/${args.workspaceId}/tasks/${args.taskId}/documents/${args.kind}`,
    {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ content: args.content }),
    },
  )
  if (!res.ok) {
    const text = await res.text().catch(() => "")
    throw new Error(
      `PUT /api/workdirs/${args.workspaceId}/tasks/${args.taskId}/documents/${args.kind} failed: ${res.status}${text ? `: ${text}` : ""}`,
    )
  }
  return (await res.json()) as TaskDocumentSnapshot
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
    res = await fetch(`/api/workdirs/${args.workspaceId}/attachments`, {
      method: "POST",
      headers: { "Idempotency-Key": resolvedKey },
      body: form,
    })
  } catch {
    await new Promise((r) => window.setTimeout(r, 200))
    res = await fetch(`/api/workdirs/${args.workspaceId}/attachments`, {
      method: "POST",
      headers: { "Idempotency-Key": resolvedKey },
      body: form,
    })
  }
  if (!res.ok) {
    const text = await res.text().catch(() => "")
    throw new Error(
      `POST /api/workdirs/${args.workspaceId}/attachments failed: ${res.status}${text ? `: ${text}` : ""}`,
    )
  }

  return (await res.json()) as AttachmentRef
}

export async function fetchWorkspaceDiff(workspaceId: number): Promise<WorkspaceDiffSnapshot> {
  if (isMockMode()) return await mockFetchWorkspaceDiff(workspaceId)
  const res = await fetch(`/api/workdirs/${workspaceId}/diff`)
  if (!res.ok) throw new Error(`GET /api/workdirs/${workspaceId}/diff failed: ${res.status}`)
  return (await res.json()) as WorkspaceDiffSnapshot
}

export async function fetchWorkspaceChanges(workspaceId: number): Promise<WorkspaceChangesSnapshot> {
  if (isMockMode()) return await mockFetchWorkspaceChanges(workspaceId)
  const res = await fetch(`/api/workdirs/${workspaceId}/changes`)
  if (!res.ok) throw new Error(`GET /api/workdirs/${workspaceId}/changes failed: ${res.status}`)
  return (await res.json()) as WorkspaceChangesSnapshot
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
    `/api/workdirs/${args.workspaceId}/mentions?q=${encodeURIComponent(q)}`,
  )
  if (!res.ok) {
    const text = await res.text().catch(() => "")
    throw new Error(
      `GET /api/workdirs/${args.workspaceId}/mentions failed: ${res.status}${text ? `: ${text}` : ""}`,
    )
  }
  return (await res.json()) as MentionItemSnapshot[]
}

export async function fetchTasks(args: {
  projectId?: string
  workdirStatus?: "active" | "archived" | "all"
  taskStatus?: TaskStatus[]
} = {}): Promise<TasksSnapshot> {
  if (isMockMode()) return await mockFetchTasks(args)
  const params = new URLSearchParams()
  if (args.projectId) params.set("project_id", args.projectId)
  if (args.workdirStatus) params.set("workdir_status", args.workdirStatus)
  if (args.taskStatus && args.taskStatus.length > 0) params.set("task_status", args.taskStatus.join(","))
  const suffix = params.toString() ? `?${params.toString()}` : ""
  const res = await fetch(`/api/tasks${suffix}`)
  if (!res.ok) throw new Error(`GET /api/tasks failed: ${res.status}`)
  return (await res.json()) as TasksSnapshot
}

export async function fetchNewTaskDrafts(): Promise<NewTaskDraftsSnapshot> {
  if (isMockMode()) return await mockFetchNewTaskDrafts()
  const res = await fetch("/api/new_task/drafts")
  if (!res.ok) throw new Error(`GET /api/new_task/drafts failed: ${res.status}`)
  return (await res.json()) as NewTaskDraftsSnapshot
}

export async function createNewTaskDraft(args: {
  text: string
  project_id: string | null
  workdir_id: number | null
}): Promise<NewTaskDraftSnapshot> {
  if (isMockMode()) return await mockCreateNewTaskDraft(args)
  const res = await fetch("/api/new_task/drafts", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(args),
  })
  if (!res.ok) throw new Error(`POST /api/new_task/drafts failed: ${res.status}`)
  return (await res.json()) as NewTaskDraftSnapshot
}

export async function updateNewTaskDraft(
  draftId: string,
  args: { text: string; project_id: string | null; workdir_id: number | null },
): Promise<NewTaskDraftSnapshot> {
  if (isMockMode()) return await mockUpdateNewTaskDraft(draftId, args)
  const res = await fetch(`/api/new_task/drafts/${encodeURIComponent(draftId)}`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(args),
  })
  if (!res.ok) throw new Error(`PUT /api/new_task/drafts/${draftId} failed: ${res.status}`)
  return (await res.json()) as NewTaskDraftSnapshot
}

export async function deleteNewTaskDraft(draftId: string): Promise<void> {
  if (isMockMode()) return await mockDeleteNewTaskDraft(draftId)
  const res = await fetch(`/api/new_task/drafts/${encodeURIComponent(draftId)}`, { method: "DELETE" })
  if (!res.ok) throw new Error(`DELETE /api/new_task/drafts/${draftId} failed: ${res.status}`)
}

export async function fetchNewTaskStash(): Promise<NewTaskStashResponse> {
  if (isMockMode()) return await mockFetchNewTaskStash()
  const res = await fetch("/api/new_task/stash")
  if (!res.ok) throw new Error(`GET /api/new_task/stash failed: ${res.status}`)
  return (await res.json()) as NewTaskStashResponse
}

export async function saveNewTaskStash(args: {
  text: string
  project_id: string | null
  workdir_id: number | null
  editing_draft_id: string | null
}): Promise<void> {
  if (isMockMode()) return await mockSaveNewTaskStash(args)
  const res = await fetch("/api/new_task/stash", {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(args),
  })
  if (!res.ok) throw new Error(`PUT /api/new_task/stash failed: ${res.status}`)
}

export async function clearNewTaskStash(): Promise<void> {
  if (isMockMode()) return await mockClearNewTaskStash()
  const res = await fetch("/api/new_task/stash", { method: "DELETE" })
  if (!res.ok) throw new Error(`DELETE /api/new_task/stash failed: ${res.status}`)
}
