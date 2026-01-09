"use client"

import type React from "react"

import { createContext, useContext, useEffect, useMemo, useRef, useState } from "react"
import { toast } from "sonner"

import type {
  AppSnapshot,
  ClientAction,
  ConversationSnapshot,
  ThreadMeta,
  WorkspaceId,
  WorkspaceSnapshot,
  WsClientMessage,
  WsServerMessage,
} from "./luban-api"
import { fetchApp, fetchConversation, fetchThreads } from "./luban-http"

const PROTOCOL_VERSION = 1

const ACTIVE_WORKSPACE_KEY = "luban:active_workspace_id"

function activeThreadKey(workspaceId: number): string {
  return `luban:active_thread_id:${workspaceId}`
}

function randomRequestId(): string {
  return `req_${Math.random().toString(16).slice(2)}_${Date.now().toString(16)}`
}

function wsUrl(path: string): string {
  const url = new URL(path, window.location.href)
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:"
  return url.toString()
}

type LubanContextValue = {
  app: AppSnapshot | null
  activeWorkspaceId: WorkspaceId | null
  activeWorkspace: WorkspaceSnapshot | null
  activeThreadId: number | null
  threads: ThreadMeta[]
  conversation: ConversationSnapshot | null
  wsConnected: boolean

  pickProjectPath: () => void
  addProject: (path: string) => void
  createWorkspace: (projectId: number) => void
  openWorkspacePullRequest: (workspaceId: WorkspaceId) => void
  openWorkspacePullRequestFailedAction: (workspaceId: WorkspaceId) => void
  archiveWorkspace: (workspaceId: number) => void
  toggleProjectExpanded: (projectId: number) => void

  openWorkspace: (workspaceId: WorkspaceId) => Promise<void>
  selectThread: (threadId: number) => Promise<void>
  createThread: () => void

  sendAgentMessage: (text: string) => void
  cancelAgentTurn: () => void
}

const LubanContext = createContext<LubanContextValue | null>(null)

export function LubanProvider({ children }: { children: React.ReactNode }) {
  const [app, setApp] = useState<AppSnapshot | null>(null)
  const [activeWorkspaceId, setActiveWorkspaceId] = useState<WorkspaceId | null>(null)
  const [activeThreadId, setActiveThreadId] = useState<number | null>(null)
  const [threads, setThreads] = useState<ThreadMeta[]>([])
  const [conversation, setConversation] = useState<ConversationSnapshot | null>(null)
  const [wsConnected, setWsConnected] = useState(false)

  const wsRef = useRef<WebSocket | null>(null)
  const pendingActionsRef = useRef<ClientAction[]>([])
  const activeWorkspaceIdRef = useRef<WorkspaceId | null>(null)
  const activeThreadIdRef = useRef<number | null>(null)
  const threadsRef = useRef<ThreadMeta[]>([])
  const pendingCreateThreadRef = useRef<{
    workspaceId: WorkspaceId
    existingThreadIds: Set<number>
    requestedAtUnixMs: number
  } | null>(null)

  useEffect(() => {
    activeWorkspaceIdRef.current = activeWorkspaceId
  }, [activeWorkspaceId])

  useEffect(() => {
    activeThreadIdRef.current = activeThreadId
  }, [activeThreadId])

  useEffect(() => {
    threadsRef.current = threads
  }, [threads])

  useEffect(() => {
    fetchApp()
      .then((snap) => setApp(snap))
      .catch((err) => console.error("fetchApp failed", err))
  }, [])

  useEffect(() => {
    const ws = new WebSocket(wsUrl("/api/events"))
    wsRef.current = ws

    ws.onopen = () => {
      const hello: WsClientMessage = {
        type: "hello",
        protocol_version: PROTOCOL_VERSION,
        last_seen_rev: null,
      }
      ws.send(JSON.stringify(hello))
      setWsConnected(true)

      const pending = pendingActionsRef.current.splice(0, pendingActionsRef.current.length)
      for (const action of pending) {
        const msg: WsClientMessage = { type: "action", request_id: randomRequestId(), action }
        ws.send(JSON.stringify(msg))
      }
    }

    ws.onmessage = (ev) => {
      if (typeof ev.data !== "string") return
      const msg = JSON.parse(ev.data) as WsServerMessage

      if (msg.type === "event") {
        const event = msg.event
        if (event.type === "app_changed") {
          setApp(event.snapshot)
          return
        }
        if (event.type === "workspace_threads_changed") {
          const wid = activeWorkspaceIdRef.current
          if (wid != null && wid === event.workspace_id) {
            setThreads(event.threads)
            const current = activeThreadIdRef.current

            const pending = pendingCreateThreadRef.current
            if (pending && pending.workspaceId === wid) {
              const created = event.threads
                .map((t) => t.thread_id)
                .filter((id) => !pending.existingThreadIds.has(id))
                .sort((a, b) => b - a)[0]
              if (created != null) {
                pendingCreateThreadRef.current = null
                void selectThreadInternal(wid, created)
                return
              }

              if (Date.now() - pending.requestedAtUnixMs > 5_000) {
                pendingCreateThreadRef.current = null
              }
            }

            if (current == null || !event.threads.some((t) => t.thread_id === current)) {
              const next = event.threads[0]?.thread_id ?? null
              if (next != null) {
                void selectThreadInternal(wid, next)
              }
            }
          }
          return
        }
        if (event.type === "conversation_changed") {
          const wid = activeWorkspaceIdRef.current
          const tid = activeThreadIdRef.current
          if (wid == null || tid == null) return
          if (event.snapshot.workspace_id === wid && event.snapshot.thread_id === tid) {
            setConversation(event.snapshot)
          }
          return
        }
        if (event.type === "toast") {
          console.warn("server toast:", event.message)
          toast(event.message)
          return
        }
      }

      if (msg.type === "error") {
        console.warn("server error:", msg.message)
        toast.error(msg.message)
      }
    }

    ws.onerror = () => setWsConnected(false)
    ws.onclose = () => setWsConnected(false)

    return () => ws.close()
  }, [])

  useEffect(() => {
    if (app == null) return
    if (activeWorkspaceId != null) return
    const raw = localStorage.getItem(ACTIVE_WORKSPACE_KEY)
    const stored = raw ? Number(raw) : null
    if (!stored || !Number.isFinite(stored)) return
    const exists = app.projects.some((p) => p.workspaces.some((w) => w.id === stored))
    if (!exists) return
    void openWorkspace(stored)
  }, [app, activeWorkspaceId])

  const activeWorkspace = useMemo(() => {
    if (!app || activeWorkspaceId == null) return null
    for (const p of app.projects) {
      const w = p.workspaces.find((x) => x.id === activeWorkspaceId)
      if (w) return w
    }
    return null
  }, [app, activeWorkspaceId])

  function sendAction(action: ClientAction) {
    const ws = wsRef.current
    if (!ws || ws.readyState !== WebSocket.OPEN) {
      if (pendingActionsRef.current.length < 128) {
        pendingActionsRef.current.push(action)
      } else {
        pendingActionsRef.current.shift()
        pendingActionsRef.current.push(action)
      }
      return
    }
    const msg: WsClientMessage = { type: "action", request_id: randomRequestId(), action }
    ws.send(JSON.stringify(msg))
  }

  async function waitForNewThread(
    workspaceId: WorkspaceId,
    existingThreadIds: Set<number>,
  ): Promise<{ threads: ThreadMeta[]; createdThreadId: number | null }> {
    const startedAt = Date.now()
    while (Date.now() - startedAt < 5_000) {
      try {
        const snap = await fetchThreads(workspaceId)
        const created = snap.threads
          .map((t) => t.thread_id)
          .filter((id) => !existingThreadIds.has(id))
          .sort((a, b) => b - a)[0]
        return { threads: snap.threads, createdThreadId: created ?? null }
      } catch {
        // ignore and retry
      }
      await new Promise((r) => setTimeout(r, 250))
    }
    return { threads: [], createdThreadId: null }
  }

  function addProject(path: string) {
    sendAction({ type: "add_project", path })
  }

  function pickProjectPath() {
    sendAction({ type: "pick_project_path" })
  }

  function createWorkspace(projectId: number) {
    sendAction({ type: "create_workspace", project_id: projectId })
  }

  function openWorkspacePullRequest(workspaceId: WorkspaceId) {
    sendAction({ type: "open_workspace_pull_request", workspace_id: workspaceId })
  }

  function openWorkspacePullRequestFailedAction(workspaceId: WorkspaceId) {
    sendAction({ type: "open_workspace_pull_request_failed_action", workspace_id: workspaceId })
  }

  function archiveWorkspace(workspaceId: number) {
    sendAction({ type: "archive_workspace", workspace_id: workspaceId })
  }

  function toggleProjectExpanded(projectId: number) {
    setApp((prev) => {
      if (!prev) return prev
      return {
        ...prev,
        projects: prev.projects.map((p) =>
          p.id === projectId ? { ...p, expanded: !p.expanded } : p,
        ),
      }
    })
    sendAction({ type: "toggle_project_expanded", project_id: projectId })
  }

  async function selectThreadInternal(workspaceId: WorkspaceId, threadId: number) {
    setActiveThreadId(threadId)
    localStorage.setItem(activeThreadKey(workspaceId), String(threadId))

    sendAction({
      type: "activate_workspace_thread",
      workspace_id: workspaceId,
      thread_id: threadId,
    })

    try {
      const convo = await fetchConversation(workspaceId, threadId)
      setConversation(convo)
    } catch (err) {
      console.warn("fetchConversation failed", err)
    }
  }

  async function openWorkspace(workspaceId: WorkspaceId) {
    setActiveWorkspaceId(workspaceId)
    localStorage.setItem(ACTIVE_WORKSPACE_KEY, String(workspaceId))
    setThreads([])
    setConversation(null)

    sendAction({ type: "open_workspace", workspace_id: workspaceId })

    try {
      const snap = await fetchThreads(workspaceId)
      setThreads(snap.threads)

      const saved = Number(localStorage.getItem(activeThreadKey(workspaceId)) ?? "")
      const initial =
        snap.threads.find((t) => t.thread_id === saved)?.thread_id ??
        snap.threads[0]?.thread_id ??
        null

      if (initial == null) {
        const existing = new Set<number>()
        pendingCreateThreadRef.current = {
          workspaceId,
          existingThreadIds: existing,
          requestedAtUnixMs: Date.now(),
        }
        sendAction({ type: "create_workspace_thread", workspace_id: workspaceId })
        setActiveThreadId(null)

        const created = await waitForNewThread(workspaceId, existing)
        if (created.createdThreadId != null) {
          pendingCreateThreadRef.current = null
          setThreads(created.threads)
          await selectThreadInternal(workspaceId, created.createdThreadId)
        }
        return
      }

      await selectThreadInternal(workspaceId, initial)
    } catch (err) {
      console.warn("fetchThreads failed", err)
    }
  }

  async function selectThread(threadId: number) {
    const wid = activeWorkspaceIdRef.current
    if (wid == null) return
    await selectThreadInternal(wid, threadId)
  }

  function createThread() {
    const wid = activeWorkspaceIdRef.current
    if (wid == null) return

    void (async () => {
      const existingThreadIds = new Set(threadsRef.current.map((t) => t.thread_id))
      pendingCreateThreadRef.current = {
        workspaceId: wid,
        existingThreadIds,
        requestedAtUnixMs: Date.now(),
      }
      sendAction({ type: "open_workspace", workspace_id: wid })
      sendAction({ type: "create_workspace_thread", workspace_id: wid })

      const created = await waitForNewThread(wid, existingThreadIds)
      if (created.createdThreadId != null) {
        pendingCreateThreadRef.current = null
        setThreads(created.threads)
        await selectThreadInternal(wid, created.createdThreadId)
      }
    })()
  }

  function sendAgentMessage(text: string) {
    const wid = activeWorkspaceIdRef.current
    const tid = activeThreadIdRef.current
    if (wid == null || tid == null) return
    sendAction({
      type: "send_agent_message",
      workspace_id: wid,
      thread_id: tid,
      text,
      attachments: [],
    })
  }

  function cancelAgentTurn() {
    const wid = activeWorkspaceIdRef.current
    const tid = activeThreadIdRef.current
    if (wid == null || tid == null) return
    sendAction({ type: "cancel_agent_turn", workspace_id: wid, thread_id: tid })
  }

  const value: LubanContextValue = {
    app,
    activeWorkspaceId,
    activeWorkspace,
    activeThreadId,
    threads,
    conversation,
    wsConnected,
    pickProjectPath,
    addProject,
    createWorkspace,
    openWorkspacePullRequest,
    openWorkspacePullRequestFailedAction,
    archiveWorkspace,
    toggleProjectExpanded,
    openWorkspace,
    selectThread,
    createThread,
    sendAgentMessage,
    cancelAgentTurn,
  }

  return <LubanContext.Provider value={value}>{children}</LubanContext.Provider>
}

export function useLuban(): LubanContextValue {
  const ctx = useContext(LubanContext)
  if (!ctx) throw new Error("useLuban must be used within LubanProvider")
  return ctx
}
