"use client"

import type { NewTaskDraftSnapshot, NewTaskStashSnapshot } from "@/lib/luban-api"
import {
  clearNewTaskStash as httpClearNewTaskStash,
  createNewTaskDraft as httpCreateNewTaskDraft,
  deleteNewTaskDraft as httpDeleteNewTaskDraft,
  fetchNewTaskDrafts,
  fetchNewTaskStash,
  saveNewTaskStash as httpSaveNewTaskStash,
  updateNewTaskDraft as httpUpdateNewTaskDraft,
} from "@/lib/luban-http"

export type NewTaskDraft = {
  id: string
  text: string
  projectId: string | null
  workdirId: number | null
  createdAtUnixMs: number
  updatedAtUnixMs: number
}

export type NewTaskStash = {
  text: string
  projectId: string | null
  workdirId: number | null
  editingDraftId: string | null
  updatedAtUnixMs: number
}

export const NEW_TASK_DRAFTS_CHANGED_EVENT = "luban:new-task-drafts-changed"

function draftFromSnapshot(value: NewTaskDraftSnapshot): NewTaskDraft {
  return {
    id: value.id,
    text: value.text,
    projectId: value.project_id,
    workdirId: value.workdir_id,
    createdAtUnixMs: value.created_at_unix_ms,
    updatedAtUnixMs: value.updated_at_unix_ms,
  }
}

function stashFromSnapshot(value: NewTaskStashSnapshot): NewTaskStash {
  return {
    text: value.text,
    projectId: value.project_id,
    workdirId: value.workdir_id,
    editingDraftId: value.editing_draft_id,
    updatedAtUnixMs: value.updated_at_unix_ms,
  }
}

export function notifyNewTaskDraftsChanged() {
  if (typeof window === "undefined") return
  window.dispatchEvent(new Event(NEW_TASK_DRAFTS_CHANGED_EVENT))
}

export async function loadNewTaskDrafts(): Promise<NewTaskDraft[]> {
  const snap = await fetchNewTaskDrafts()
  return (snap.drafts ?? []).map(draftFromSnapshot)
}

export async function upsertNewTaskDraft(args: {
  id?: string | null
  text: string
  projectId: string | null
  workdirId: number | null
}): Promise<NewTaskDraft> {
  const payload = { text: args.text, project_id: args.projectId, workdir_id: args.workdirId }

  const snap = args.id
    ? await httpUpdateNewTaskDraft(args.id, payload)
    : await httpCreateNewTaskDraft(payload)

  notifyNewTaskDraftsChanged()
  return draftFromSnapshot(snap)
}

export async function deleteNewTaskDraft(id: string): Promise<void> {
  await httpDeleteNewTaskDraft(id)
  notifyNewTaskDraftsChanged()
}

export async function loadNewTaskStash(): Promise<NewTaskStash | null> {
  const resp = await fetchNewTaskStash()
  return resp.stash ? stashFromSnapshot(resp.stash) : null
}

export async function saveNewTaskStash(value: NewTaskStash): Promise<void> {
  await httpSaveNewTaskStash({
    text: value.text,
    project_id: value.projectId,
    workdir_id: value.workdirId,
    editing_draft_id: value.editingDraftId,
  })
}

export async function clearNewTaskStash(): Promise<void> {
  await httpClearNewTaskStash()
}

